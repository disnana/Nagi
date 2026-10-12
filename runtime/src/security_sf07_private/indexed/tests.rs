use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
fn lim(n: usize) -> Limits {
    Limits {
        count: n + 1,
        bytes: 16 * 1024 * 1024,
        work: 50000,
    }
}
fn owner(n: usize, burst: u32, refill: u64, idle: u64) -> RateOwner {
    RateOwner::checked_rate(&[Rule::new(burst, refill, Slots::Subject(n), idle)], lim(n)).unwrap()
}
fn key(v: i64) -> FixedKey {
    FixedKey::subject(DomainId(1), v)
}
fn run(o: &RateOwner, v: i64, t: u64) -> RateDecision {
    o.charge_at_with_test_clock(o.rule_id(0).unwrap(), key(v), t)
}
fn entries(o: &RateOwner) -> Vec<Option<Entry>> {
    o.state.lock().unwrap().rules[0].subjects.entries.clone()
}
fn tag(d: RateDecision) -> (u8, u64) {
    match d {
        RateDecision::Charged => (0, 0),
        RateDecision::Limited { retry_ns } => (1, retry_ns),
        RateDecision::Capacity => (2, 0),
        RateDecision::Closing => (3, 0),
        RateDecision::ClockFault => (4, 0),
        RateDecision::Fault => (5, 0),
    }
}

#[test]
fn foreign_brand_and_exhausted_brand_do_not_admit() {
    let a = owner(2, 1, 10, 1);
    let b = owner(2, 1, 10, 1);
    let aid = a.rule_id(0).unwrap();
    assert_eq!(
        b.charge_at_with_test_clock(aid, key(1), 0),
        RateDecision::Fault
    );
    assert!(entries(&b).iter().all(Option::is_none));
    assert_eq!(run(&b, 1, 0), RateDecision::Fault);
    let stale = aid;
    drop(a);
    let c = owner(2, 1, 10, 1);
    assert_ne!(stale.brand, c.brand);
    assert_eq!(
        c.charge_at_with_test_clock(stale, key(1), 0),
        RateDecision::Fault
    );
    for v in [0, u64::MAX] {
        let counter = AtomicU64::new(v);
        assert_eq!(fresh_brand(&counter), Err(StartupError::Invalid));
        assert_eq!(counter.load(Ordering::Relaxed), v);
    }
}
#[test]
fn exact_count_bytes_work_and_native_overflow() {
    let r = [Rule::new(1, 10, Slots::Subject(3), 1)];
    let o = RateOwner::checked_rate(&r, lim(3)).unwrap();
    let exact = Limits {
        count: 4,
        bytes: o.retained_bytes,
        work: o.certificate,
    };
    assert!(RateOwner::checked_rate(&r, exact).is_ok());
    for bad in [
        Limits { count: 3, ..exact },
        Limits {
            bytes: exact.bytes - 1,
            ..exact
        },
        Limits {
            work: exact.work - 1,
            ..exact
        },
        Limits { count: 0, ..exact },
        Limits { bytes: 0, ..exact },
        Limits { work: 0, ..exact },
    ] {
        assert!(matches!(
            RateOwner::checked_rate(&r, bad),
            Err(StartupError::Invalid)
        ));
    }
    assert!(layout(usize::MAX).is_none());
    assert!(array_bytes::<Entry>(usize::MAX).is_none());
    assert!(RateOwner::checked_rate(
        &[Rule::new(1, 1, Slots::Subject(usize::MAX), 1)],
        Limits {
            count: usize::MAX,
            bytes: usize::MAX,
            work: u64::MAX
        }
    )
    .is_err());
    for arena in 0..4 {
        let result = RateOwner::checked_with(&r, exact, |s| match arena {
            0 => s.try_reserve_exact(1).unwrap(),
            1 => s[0].subjects.entries.try_reserve_exact(1).unwrap(),
            2 => s[0].subjects.branches.try_reserve_exact(1).unwrap(),
            _ => s[0].subjects.summary.try_reserve_exact(1).unwrap(),
        });
        assert!(matches!(result, Err(StartupError::Invalid)));
    }
}
#[test]
fn empty_first_padding_and_leftmost_not_earliest_due() {
    let o = owner(3, 2, 10, 5);
    run(&o, 1, 0);
    assert_eq!(run(&o, 2, 20), RateDecision::Charged);
    let e = entries(&o);
    assert_eq!(e[0].unwrap().key.1, 1);
    assert_eq!(e[1].unwrap().key.1, 2);
    let o = owner(2, 2, 10, 5);
    run(&o, 1, 0);
    run(&o, 2, 1);
    run(&o, 1, 2);
    assert_eq!(entries(&o)[0].unwrap().due, 20);
    assert_eq!(entries(&o)[1].unwrap().due, 11);
    assert_eq!(run(&o, 3, 20), RateDecision::Charged);
    let e = entries(&o);
    assert_eq!(e[0].unwrap().key.1, 3);
    assert_eq!(e[1].unwrap().key.1, 2);
    let o = owner(3, 1, 10, 1);
    for i in 0..3 {
        assert_eq!(run(&o, i, 0), RateDecision::Charged);
    }
    assert_eq!(run(&o, 4, 0), RateDecision::Capacity);
    assert_eq!(
        o.state.lock().unwrap().rules[0].subjects.summary[7],
        Summary::default()
    );
}
#[test]
fn full_tree_replacement_reuses_branch_and_single_slot() {
    for n in [1, 2, 3, 7, 8] {
        let o = owner(n, 1, 10, 1);
        for i in 0..n {
            assert_eq!(run(&o, i as i64, 0), RateDecision::Charged);
        }
        for t in 1..40 {
            assert_eq!(run(&o, 100 + t, t as u64 * 10), RateDecision::Charged);
        }
        let s = o.state.lock().unwrap();
        assert_eq!(
            s.rules[0]
                .subjects
                .branches
                .iter()
                .filter(|b| b.active)
                .count(),
            n - 1
        );
        assert_eq!(s.rules[0].subjects.free_branch, None);
    }
}
#[test]
fn key_width_endpoints_and_shared_last_bit() {
    let o = owner(6, 1, 100, 100);
    let id = o.rule_id(0).unwrap();
    let keys = [
        (0, 0),
        (0, 1),
        (0, i64::MIN),
        (0, i64::MAX),
        (u16::MAX, i64::MIN),
        (u16::MAX, i64::MAX),
    ];
    for (d, v) in keys {
        assert_eq!(
            o.charge_at_with_test_clock(id, FixedKey::subject(DomainId(d), v), 0),
            RateDecision::Charged
        );
    }
    for (d, v) in keys {
        assert_eq!(
            o.charge_at_with_test_clock(id, FixedKey::subject(DomainId(d), v), 0),
            RateDecision::Limited { retry_ns: 100 }
        );
    }
    assert!(o.state.lock().unwrap().rules[0]
        .subjects
        .branches
        .iter()
        .any(|b| b.active && b.bit == 79));
}
#[test]
fn certificate_above_u64_and_exact_idle() {
    let o = owner(1, u32::MAX, u64::MAX, u64::MAX);
    assert_eq!(run(&o, 1, u64::MAX - 1), RateDecision::Charged);
    assert!(entries(&o)[0].unwrap().due > u128::from(u64::MAX));
    assert_eq!(run(&o, 2, u64::MAX), RateDecision::Capacity);
    let o = owner(1, 1, 10, 20);
    run(&o, 1, 0);
    assert_eq!(run(&o, 2, 19), RateDecision::Capacity);
    assert_eq!(run(&o, 2, 20), RateDecision::Charged);
}
#[test]
fn runtime_guard_rolls_back_every_cut_and_stays_faulted() {
    let sample = owner(2, 1, 10, 1);
    run(&sample, 1, 0);
    run(&sample, 2, 0);
    run(&sample, 3, 10);
    let end = sample.state.lock().unwrap().last_work;
    for cut in 0..end {
        let o = owner(2, 1, 10, 1);
        run(&o, 1, 0);
        run(&o, 2, 0);
        let before = {
            let mut s = o.state.lock().unwrap();
            let r = &s.rules[0].subjects;
            let saved = (
                r.entries.clone(),
                r.branches.clone(),
                r.summary.clone(),
                r.root,
                r.free_branch,
            );
            s.work_cap = CLEANUP_WORK + cut;
            saved
        };
        assert_eq!(run(&o, 3, 10), RateDecision::Fault, "cut {cut}");
        {
            let s = o.state.lock().unwrap();
            let r = &s.rules[0].subjects;
            assert_eq!(
                (
                    r.entries.clone(),
                    r.branches.clone(),
                    r.summary.clone(),
                    r.root,
                    r.free_branch
                ),
                before
            );
            assert!(s.last_work <= s.work_cap);
        }
        assert_eq!(run(&o, 1, 20), RateDecision::Fault);
    }
}
#[test]
fn panic_after_mutations_restores_debt_and_poison_closes() {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut panics = 0;
    let sample = owner(2, 1, 10, 1);
    run(&sample, 1, 0);
    run(&sample, 2, 0);
    run(&sample, 3, 10);
    let end = sample.state.lock().unwrap().last_work;
    for tick in 49..=end {
        let o = owner(2, 1, 10, 1);
        run(&o, 1, 0);
        run(&o, 2, 0);
        let before = entries(&o);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut s = o.state.lock().unwrap();
            let mut w = Work::new(50000).unwrap();
            w.used = 32;
            w.panic_after = Some(tick);
            charge_rule(&mut s.rules[0], key(3), 10, &mut w)
        }));
        if result.is_err() {
            panics += 1;
            let s = match o.state.lock() {
                Err(p) => p.into_inner(),
                Ok(_) => panic!("expected poison"),
            };
            assert_eq!(s.rules[0].subjects.entries, before);
            drop(s);
            assert_eq!(run(&o, 1, 20), RateDecision::Fault);
        }
    }
    std::panic::set_hook(old_hook);
    assert!(panics > 20);
}
#[test]
fn detected_summary_link_and_freelist_corruption_faults() {
    for kind in 0..3 {
        let o = owner(3, 1, 10, 1);
        run(&o, 1, 0);
        {
            let mut s = o.state.lock().unwrap();
            let r = &mut s.rules[0].subjects;
            match kind {
                0 => r.summary[1] = Summary::default(),
                1 => r.root = Link::Branch(usize::MAX),
                _ => r.free_branch = None,
            }
        }
        let before = entries(&o);
        assert_eq!(run(&o, 2, 10), RateDecision::Fault);
        assert_eq!(entries(&o), before);
        assert_eq!(run(&o, 1, 20), RateDecision::Fault);
    }
}
#[test]
fn differential_semantic_decisions_and_retired_identities() {
    for n in 1..=9 {
        for burst in [1, 2, 3] {
            for (refill, idle) in [(1, 1), (7, 9)] {
                let o = owner(n, burst, refill, idle);
                let old = super::super::legacy_rate::RateOwner::checked_rate(
                    &[super::super::legacy_rate::Rule::new(
                        burst,
                        refill,
                        super::super::legacy_rate::Slots::Subject(n),
                        idle,
                    )],
                    1000000,
                )
                .unwrap();
                let mut random = 5u64;
                let mut now = 0;
                for _ in 0..400 {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    now += random % 4;
                    let domain = ((random >> 8) % 3) as u16;
                    let v = (((random >> 16) % (n as u64 * 3)) as i64) - n as i64;
                    let actual = o.charge_at_with_test_clock(
                        o.rule_id(0).unwrap(),
                        FixedKey::subject(DomainId(domain), v),
                        now,
                    );
                    let expected = super::super::legacy_rate::bridge_charge(&old, domain, v, now);
                    assert_eq!(tag(actual), expected);
                    let actual = entries(&o)
                        .iter()
                        .map(|e| e.map(|e| (e.key.0 .0, e.key.1)))
                        .collect::<Vec<_>>();
                    assert_eq!(actual, super::super::legacy_rate::bridge_keys(&old));
                }
            }
        }
    }
}
#[test]
fn request_transitions_allocate_zero_and_native_profiles_fit() {
    eprintln!("SF07_DIAG phase=calibration_before");
    super::super::calibrate_allocation_meter();
    eprintln!("SF07_DIAG phase=calibration_after");
    let decision_label = |decision| match decision {
        RateDecision::Charged => "charged",
        RateDecision::Limited { .. } => "limited",
        RateDecision::Capacity => "capacity",
        RateDecision::Closing => "closing",
        RateDecision::ClockFault => "clock_fault",
        RateDecision::Fault => "fault",
    };
    for n in [32, 128, 4096, 4097, 8192] {
        eprintln!("SF07_DIAG phase=profile_start profile_n={n}");
        let o = owner(n, 2, 10, 1);
        for i in 0..4 {
            let (decision, allocs) = super::super::allocations_in(|| run(&o, i, 0));
            if decision != RateDecision::Charged {
                eprintln!(
                    "SF07_DIAG phase=transition_decision profile_n={n} index={i} actual={} expected=charged",
                    decision_label(decision)
                );
            }
            assert_eq!(decision, RateDecision::Charged);
            if allocs != 0 {
                eprintln!(
                    "SF07_DIAG phase=transition_allocations profile_n={n} index={i} actual={allocs} expected=0"
                );
            }
            assert_eq!(allocs, 0);
        }
        let (decision, allocs) = super::super::allocations_in(|| run(&o, 0, 0));
        if decision != RateDecision::Charged {
            eprintln!(
                "SF07_DIAG phase=repeat_decision profile_n={n} index=0 actual={} expected=charged",
                decision_label(decision)
            );
        }
        assert_eq!(decision, RateDecision::Charged);
        if allocs != 0 {
            eprintln!(
                "SF07_DIAG phase=repeat_allocations profile_n={n} index=0 actual={allocs} expected=0"
            );
        }
        assert_eq!(allocs, 0);
        if o.retained_bytes >= 4 * 1024 * 1024 {
            eprintln!(
                "SF07_DIAG phase=native_profile_size profile_n={n} actual_bytes={} expected_upper_exclusive=4194304",
                o.retained_bytes
            );
        }
        assert!(o.retained_bytes < 4 * 1024 * 1024);
    }
}

#[test]
fn all_eighty_patricia_positions_and_full_replacement() {
    let n = 81;
    let o = owner(n, 2, 10, 1);
    let id = o.rule_id(0).unwrap();
    assert_eq!(
        o.charge_at_with_test_clock(id, FixedKey::subject(DomainId(0), 0), 0),
        RateDecision::Charged
    );
    for bit in 0..KEY_BITS {
        let (d, v) = if bit < 16 {
            (1u16 << (15 - bit), 0)
        } else {
            (0, (1u64 << (79 - bit)) as i64)
        };
        assert_eq!(
            o.charge_at_with_test_clock(id, FixedKey::subject(DomainId(d), v), 0),
            RateDecision::Charged
        );
    }
    assert_eq!(
        o.charge_at_with_test_clock(id, FixedKey::subject(DomainId(0), 0), 0),
        RateDecision::Charged
    );
    {
        let s = o.state.lock().unwrap();
        assert_eq!(
            s.rules[0]
                .subjects
                .branches
                .iter()
                .filter(|b| b.active)
                .count(),
            KEY_BITS
        );
        assert!(s.last_work <= o.certificate);
    }
    assert_eq!(
        o.charge_at_with_test_clock(id, FixedKey::subject(DomainId(u16::MAX), i64::MAX), 20),
        RateDecision::Charged
    );
    assert_eq!(entries(&o)[0].unwrap().key, (DomainId(u16::MAX), i64::MAX));
}

mod inherited {

    use super::*;
    fn limits(count: usize, bytes: usize) -> Limits {
        Limits {
            count,
            bytes,
            work: 50000,
        }
    }
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, TryLockError};
    use std::time::Duration;

    fn owner(burst: u32, refill_ns: u64, slots: Slots, idle_ns: u64) -> RateOwner {
        RateOwner::checked_rate(
            &[Rule::new(burst, refill_ns, slots, idle_ns)],
            limits(4097, 1_000_000),
        )
        .expect("valid fixed rules")
    }

    fn charge_at(owner: &RateOwner, rule: RuleId, key: FixedKey, now: u64) -> RateDecision {
        let mut state = owner.state.lock().expect("unpoisoned test state");
        state.charge_at(rule, key, now)
    }

    fn subject(domain: u16, value: i64) -> FixedKey {
        FixedKey::subject(DomainId::from_trusted_config(domain), value)
    }

    #[test]
    fn r01_burst_two_refills_exactly_one_token_per_interval() {
        let owner = owner(2, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let actual = [
            charge_at(&owner, rule, FixedKey::Route, 0),
            charge_at(&owner, rule, FixedKey::Route, 0),
            charge_at(&owner, rule, FixedKey::Route, 0),
            charge_at(&owner, rule, FixedKey::Route, 10),
            charge_at(&owner, rule, FixedKey::Route, 10),
        ];
        assert_eq!(
            actual,
            [
                RateDecision::Charged,
                RateDecision::Charged,
                RateDecision::Limited { retry_ns: 10 },
                RateDecision::Charged,
                RateDecision::Limited { retry_ns: 10 },
            ]
        );
    }

    #[test]
    fn r06_depleted_subject_debt_cannot_be_evicted() {
        let owner = owner(2, 100, Slots::Subject(1), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 2),
            RateDecision::Capacity
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 2),
            RateDecision::Limited { retry_ns: 98 }
        );
    }

    #[test]
    fn r07_full_refill_and_idle_allow_one_fixed_slot_reuse() {
        let owner = owner(2, 10, Slots::Subject(1), 5);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 20),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 20),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 20),
            RateDecision::Limited { retry_ns: 10 }
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(
            state.rules[rule.index]
                .subjects
                .entries
                .iter()
                .flatten()
                .count(),
            1
        );
        assert_eq!(state.rules[rule.index].subjects.entries.capacity(), 1);
    }

    #[test]
    fn r08_partial_refill_is_not_evicted_or_reset() {
        let owner = owner(2, 10, Slots::Subject(1), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 10),
            RateDecision::Capacity
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 10),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 10),
            RateDecision::Limited { retry_ns: 10 }
        );
    }

    #[test]
    fn r11_subject_table_stays_fixed_and_refuses_untracked_subjects() {
        let owner = owner(1, 10, Slots::Subject(2), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 0),
            RateDecision::Charged
        );
        let before = {
            let state = owner.state.lock().expect("unpoisoned test state");
            state.rules[rule.index].subjects.entries.capacity()
        };
        assert_eq!(
            charge_at(&owner, rule, subject(1, 3), 0),
            RateDecision::Capacity
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.rules[rule.index].subjects.entries.capacity(), before);
        assert_eq!(
            state.rules[rule.index]
                .subjects
                .entries
                .iter()
                .flatten()
                .count(),
            2
        );
    }

    #[test]
    fn production_clock_path_and_close_are_usable() {
        let owner = owner(1, 1_000_000_000_000, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(owner.charge(rule, FixedKey::Route), RateDecision::Charged);
        assert!(matches!(
            owner.charge(rule, FixedKey::Route),
            RateDecision::Limited { retry_ns } if retry_ns > 0 && retry_ns <= 1_000_000_000_000
        ));
        owner.close();
        assert_eq!(owner.charge(rule, FixedKey::Route), RateDecision::Closing);
    }

    #[test]
    fn r02_refill_boundary_minus_one_then_exact_interval() {
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 9),
            RateDecision::Limited { retry_ns: 1 }
        );
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 10),
            RateDecision::Charged
        );
    }

    #[test]
    fn r03_huge_elapsed_saturates_before_token_subtraction() {
        let huge_elapsed_owner = owner(2, 1, Slots::NoSubject, 1);
        let rule = huge_elapsed_owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&huge_elapsed_owner, rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&huge_elapsed_owner, rule, FixedKey::Route, u64::MAX),
            RateDecision::Charged
        );
        let state = huge_elapsed_owner
            .state
            .lock()
            .expect("unpoisoned test state");
        let bucket = state.rules[0].route.expect("route bucket retained");
        assert_eq!(bucket.tokens, 1);
        assert_eq!(bucket.anchor, u64::MAX);
        assert_eq!(bucket.last_charge, u64::MAX);

        let long_interval = owner(1, u64::MAX, Slots::NoSubject, 1);
        let long_rule = long_interval.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&long_interval, long_rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&long_interval, long_rule, FixedKey::Route, 0),
            RateDecision::Limited { retry_ns: u64::MAX }
        );
        assert_eq!(
            charge_at(&long_interval, long_rule, FixedKey::Route, u64::MAX),
            RateDecision::Charged
        );
    }

    #[test]
    fn r04_zero_and_oversized_rule_settings_fail_startup() {
        assert!(RateOwner::checked_rate(
            &[Rule::new(0, 1, Slots::NoSubject, 1)],
            limits(4097, 1_000_000)
        )
        .is_err());
        assert!(RateOwner::checked_rate(
            &[Rule::new(1, 0, Slots::NoSubject, 1)],
            limits(4097, 1_000_000)
        )
        .is_err());
        assert!(RateOwner::checked_rate(
            &[Rule::new(1, 1, Slots::Subject(0), 1)],
            limits(4097, 1_000_000)
        )
        .is_err());
        assert!(RateOwner::checked_rate(
            &[Rule::new(1, 1, Slots::Subject(4097), 1)],
            limits(4097, 1_000_000)
        )
        .is_err());
        assert!(RateOwner::checked_rate(
            &[
                Rule::new(1, 1, Slots::Subject(2049), 1),
                Rule::new(1, 1, Slots::Subject(2049), 1),
            ],
            limits(4097, 1_000_000),
        )
        .is_err());
        assert!(owner(1, 1, Slots::NoSubject, 1).rule_id(0).is_some());
    }

    #[test]
    fn r05_denials_do_not_slide_or_reset_bucket_debt() {
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        for (now, retry) in [(1, 9), (2, 8), (8, 2)] {
            assert_eq!(
                charge_at(&owner, rule, FixedKey::Route, now),
                RateDecision::Limited { retry_ns: retry }
            );
            let state = owner.state.lock().expect("unpoisoned test state");
            let bucket = state.rules[0].route.expect("route bucket retained");
            assert_eq!(bucket.tokens, 0);
            assert_eq!(bucket.anchor, 0);
            assert_eq!(bucket.last_charge, 0);
        }
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 10),
            RateDecision::Charged
        );
    }

    #[test]
    fn r09_subject_buckets_are_separated_by_trusted_domain_id() {
        let owner = owner(1, 10, Slots::Subject(2), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 7), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(2, 7), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 7), 0),
            RateDecision::Limited { retry_ns: 10 }
        );
        assert_eq!(
            charge_at(&owner, rule, subject(2, 7), 0),
            RateDecision::Limited { retry_ns: 10 }
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        let keys = state.rules[0]
            .subjects
            .entries
            .iter()
            .filter_map(Option::as_ref)
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        assert!(keys.contains(&(DomainId::from_trusted_config(1), 7)));
        assert!(keys.contains(&(DomainId::from_trusted_config(2), 7)));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn r12_backward_clock_fault_is_sticky_and_preserves_bucket() {
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 10),
            RateDecision::Charged
        );
        let before = {
            let state = owner.state.lock().expect("unpoisoned test state");
            let bucket = state.rules[0].route.expect("route bucket retained");
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now,
            )
        };
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 9),
            RateDecision::ClockFault
        );
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 20),
            RateDecision::ClockFault
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        let bucket = state.rules[0].route.expect("route bucket retained");
        assert_eq!(
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now
            ),
            before
        );
        assert!(state.clock_fault);
    }

    #[test]
    fn r13_elapsed_conversion_failure_is_sticky_without_refill_or_charge() {
        assert_eq!(checked_elapsed_ns(Duration::from_secs(u64::MAX)), None);
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            owner.charge_at_with_test_clock(rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        let before = {
            let state = owner.state.lock().expect("unpoisoned test state");
            let bucket = state.rules[0].route.expect("route bucket retained");
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now,
            )
        };
        assert_eq!(
            owner.charge_with_clock(rule, FixedKey::Route, || None),
            RateDecision::ClockFault
        );
        let should_not_sample = Cell::new(false);
        assert_eq!(
            owner.charge_with_clock(rule, FixedKey::Route, || {
                should_not_sample.set(true);
                Some(20)
            }),
            RateDecision::ClockFault
        );
        assert!(!should_not_sample.get());
        let state = owner.state.lock().expect("unpoisoned test state");
        let bucket = state.rules[0].route.expect("route bucket retained");
        assert_eq!(
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now
            ),
            before
        );
        assert!(state.clock_fault);
    }

    #[test]
    fn r14_controlled_clock_is_sampled_under_transition_lock() {
        let owner = Arc::new(owner(2, 10, Slots::NoSubject, 1));
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let barrier = Arc::new(Barrier::new(3));
        let next_tick = Arc::new(AtomicU64::new(0));
        let threads = (0..2)
            .map(|_| {
                let owner = Arc::clone(&owner);
                let barrier = Arc::clone(&barrier);
                let next_tick = Arc::clone(&next_tick);
                std::thread::spawn(move || {
                    barrier.wait();
                    owner.charge_with_clock(rule, FixedKey::Route, || {
                        assert!(matches!(
                            owner.state.try_lock(),
                            Err(TryLockError::WouldBlock)
                        ));
                        Some(next_tick.fetch_add(1, Ordering::SeqCst))
                    })
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let decisions = threads
            .into_iter()
            .map(|thread| thread.join().expect("clock worker completed"))
            .collect::<Vec<_>>();
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == RateDecision::Charged)
                .count(),
            2
        );
        assert_eq!(next_tick.load(Ordering::SeqCst), 2);
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.last_now, Some(1));
        assert!(!state.clock_fault);
        assert_eq!(
            state.rules[0].route.expect("route bucket retained").tokens,
            0
        );
    }

    #[test]
    fn r15_arc_handles_share_one_owner_but_separate_owner_has_fresh_state() {
        let shared = Arc::new(owner(1, 1_000_000_000_000, Slots::NoSubject, 1));
        let first_handle = Arc::clone(&shared);
        let second_handle = Arc::clone(&shared);
        let rule = shared.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            first_handle.charge(rule, FixedKey::Route),
            RateDecision::Charged
        );
        assert!(matches!(
            second_handle.charge(rule, FixedKey::Route),
            RateDecision::Limited { .. }
        ));
        let separate = owner(1, 1_000_000_000_000, Slots::NoSubject, 1);
        let separate_rule = separate.rule_id(0).expect("separate fixed rule ID");
        assert_eq!(
            separate.charge(separate_rule, FixedKey::Route),
            RateDecision::Charged
        );
    }

    #[test]
    fn r16_close_preserves_debt_and_prevents_sampling_or_new_key() {
        let owner = owner(1, 10, Slots::Subject(1), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let key = subject(1, 7);
        assert_eq!(
            owner.charge_at_with_test_clock(rule, key, 1),
            RateDecision::Charged
        );
        owner.close();
        let sampled = Cell::new(false);
        assert_eq!(
            owner.charge_with_clock(rule, subject(1, 8), || {
                sampled.set(true);
                Some(100)
            }),
            RateDecision::Closing
        );
        assert!(!sampled.get());
        let state = owner.state.lock().expect("unpoisoned test state");
        let bucket = state.rules[0].subjects.entries[0]
            .expect("original subject retained")
            .bucket;
        assert_eq!(
            (bucket.tokens, bucket.anchor, bucket.last_charge),
            (0, 1, 1)
        );
        assert_eq!(
            state.rules[rule.index]
                .subjects
                .entries
                .iter()
                .flatten()
                .count(),
            1
        );
        assert_eq!(state.last_now, Some(1));
        assert!(state.closing);
    }

    #[test]
    fn r18_three_simultaneous_same_tick_charges_accept_exact_burst() {
        let owner = Arc::new(owner(2, 10, Slots::NoSubject, 1));
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let barrier = Arc::new(Barrier::new(4));
        let threads = (0..3)
            .map(|_| {
                let owner = Arc::clone(&owner);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    owner.charge_at_with_test_clock(rule, FixedKey::Route, 7)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let decisions = threads
            .into_iter()
            .map(|thread| thread.join().expect("charge worker completed"))
            .collect::<Vec<_>>();
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == RateDecision::Charged)
                .count(),
            2
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == RateDecision::Limited { retry_ns: 10 })
                .count(),
            1
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.last_now, Some(7));
        assert_eq!(
            state.rules[0].route.expect("route bucket retained").tokens,
            0
        );
    }
}

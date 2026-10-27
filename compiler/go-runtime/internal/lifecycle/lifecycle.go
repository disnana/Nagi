// Package lifecycle is a private experimental supervision kernel. It is not a
// std.actor implementation: no Nagi program can select these Go callbacks yet.
package lifecycle

import (
	"context"
	"errors"
	"strings"
	"sync"
	"time"
	"unicode/utf8"
)

type Phase uint8

const (
	Registered Phase = iota
	Running
	Stopping
	Cleaning
	Complete
)

type Policy uint8

const (
	Temporary Policy = iota
	Transient
	Permanent
)

var (
	ErrIncomplete      = errors.New("supervisor cleanup incomplete")
	ErrPanicked        = errors.New("registered child panicked")
	ErrCleanupPanicked = errors.New("supervisor cleanup panicked")
	ErrIntensity       = errors.New("supervisor restart intensity exceeded")
)

type Options struct {
	Children        int
	Restarts        int
	Window          time.Duration
	RestartDelay    time.Duration
	ShutdownTimeout time.Duration
}

// Factory runs inside the retained child goroutine on every generation. A nil
// result is successful worker completion; an error is a worker failure. Business
// replies do not cross this boundary and must not be returned as worker errors.
type Factory func(context.Context) error

type slot struct {
	name       string
	policy     Policy
	factory    Factory
	generation uint64
	cancel     context.CancelFunc
	active     bool
	pending    bool
	restartAt  time.Time
}

type completion struct {
	index      int
	generation uint64
	err        error
	panicked   bool
	canceled   bool
}

// Snapshot counts owned logical obligations, not GC reachability or physical
// memory reclamation. A completed hook is the only cleanup evidence here.
type Snapshot struct {
	Phase             Phase
	ActiveRecords     int
	PendingRestarts   int
	CleanupActive     bool
	Starts            uint64
	ScheduledRestarts uint64
	WorkerFailures    uint64
	Panics            uint64
	Generations       []uint64
}

type Group struct {
	mu                                 sync.Mutex
	options                            Options
	phase                              Phase
	started                            bool
	slots                              []slot
	active                             int
	cleanup                            func() error
	cleanupActive                      bool
	firstFailure                       error
	history                            []time.Time
	starts, restarts, failures, panics uint64
	wake                               chan struct{}
	finished                           chan completion
	done                               chan struct{}
}

func New(options Options, cleanup func() error) (*Group, error) {
	if options.Children < 1 || options.Children > 65536 || options.Restarts < 0 || options.Restarts > 65536 || options.Window <= 0 || options.RestartDelay < 0 || options.ShutdownTimeout <= 0 {
		return nil, errors.New("invalid supervisor options")
	}
	return &Group{options: options, phase: Registered, cleanup: cleanup,
		wake: make(chan struct{}, 1), finished: make(chan completion, options.Children), done: make(chan struct{})}, nil
}

func (g *Group) Register(name string, policy Policy, factory Factory) error {
	g.mu.Lock()
	defer g.mu.Unlock()
	if g.phase != Registered || len(g.slots) >= g.options.Children || len(name) < 1 || len(name) > 128 || !utf8.ValidString(name) || policy > Permanent || factory == nil {
		return errors.New("invalid child registration")
	}
	for _, child := range g.slots {
		if child.name == name {
			return errors.New("duplicate child name")
		}
	}
	g.slots = append(g.slots, slot{name: strings.Clone(name), policy: policy, factory: factory})
	return nil
}

func (g *Group) Start() error {
	g.mu.Lock()
	if g.started || g.phase != Registered {
		g.mu.Unlock()
		return errors.New("supervisor can only start once")
	}
	g.started = true
	g.phase = Running
	g.mu.Unlock()
	go g.manage()
	return nil
}

func (g *Group) notify() {
	select {
	case g.wake <- struct{}{}:
	default:
	}
}

// RequestStop is idempotent. Cancellation is only a request: records remain
// owned until each factory and its deferred cleanup have actually returned.
func (g *Group) RequestStop() {
	g.mu.Lock()
	if g.phase >= Cleaning {
		g.mu.Unlock()
		return
	}
	g.phase = Stopping
	var cancels []context.CancelFunc
	for i := range g.slots {
		g.slots[i].pending = false
		if g.slots[i].cancel != nil {
			cancels = append(cancels, g.slots[i].cancel)
		}
	}
	start := !g.started
	g.started = true
	g.mu.Unlock()
	for _, cancel := range cancels {
		cancel()
	}
	if start {
		go g.manage()
	}
	g.notify()
}

func (g *Group) Wait(ctx context.Context) error {
	select {
	case <-g.done:
	case <-ctx.Done():
		select {
		case <-g.done:
		default:
			return ErrIncomplete
		}
	}
	g.mu.Lock()
	defer g.mu.Unlock()
	return g.firstFailure
}

func (g *Group) StopAndWait(ctx context.Context) error {
	g.RequestStop()
	bounded, cancel := context.WithTimeout(ctx, g.options.ShutdownTimeout)
	defer cancel()
	return g.Wait(bounded)
}

func (g *Group) Snapshot() Snapshot {
	g.mu.Lock()
	defer g.mu.Unlock()
	s := Snapshot{Phase: g.phase, ActiveRecords: g.active, CleanupActive: g.cleanupActive,
		Starts: g.starts, ScheduledRestarts: g.restarts, WorkerFailures: g.failures, Panics: g.panics}
	for _, child := range g.slots {
		s.Generations = append(s.Generations, child.generation)
		if child.pending {
			s.PendingRestarts++
		}
	}
	return s
}

// invoke does not format arbitrary panic payloads or invoke their methods.
// recover covers only panics in this goroutine, never OOM/abort/os.Exit/native
// fatal faults or a separately launched goroutine.
func invoke(factory Factory, ctx context.Context) (err error, panicked bool) {
	returned := false
	defer func() {
		if !returned {
			_ = recover()
			err = ErrPanicked
			panicked = true
		}
	}()
	err = boundedError(factory(ctx))
	returned = true
	return
}

// Never retain an arbitrary native error object as a terminal cause. Its Error
// method runs within the child/cleanup retention boundary and outside locks;
// a panic or blocking method is accounted for just like the callback itself.
func boundedError(err error) error {
	if err == nil || err == context.Canceled {
		return err
	}
	message := err.Error()
	if len(message) > 1024 {
		message = message[:1024]
		for len(message) > 0 && !utf8.ValidString(message) {
			message = message[:len(message)-1]
		}
	}
	return errors.New(strings.Clone(message))
}

func (g *Group) launch(index int) {
	g.mu.Lock()
	if g.phase != Running {
		g.mu.Unlock()
		return
	}
	child := &g.slots[index]
	child.pending = false
	child.generation++
	child.active = true
	ctx, cancel := context.WithCancel(context.Background())
	child.cancel = cancel
	g.active++
	g.starts++
	factory, generation := child.factory, child.generation
	g.mu.Unlock()
	go func() {
		err, panicked := ctx.Err(), false
		if err == nil {
			err, panicked = invoke(factory, ctx)
		}
		// Freeze the outcome before our own cancel and before manager queueing.
		// A later stop must not turn an already finished worker error into a
		// cancellation merely because its record has not been drained yet.
		canceled := err == context.Canceled && ctx.Err() != nil && !panicked
		cancel()
		// invoke has returned after all user defers, including panic unwinding.
		g.finished <- completion{index, generation, err, panicked, canceled}
	}()
}

func (g *Group) remember(err error) {
	if g.firstFailure == nil {
		g.firstFailure = err
	}
}

// Restart decisions are serialized by the manager. Initial starts are free;
// scheduled attempts share the exact sliding window (now-window, now].
func expire(history []time.Time, now time.Time, window time.Duration) []time.Time {
	for len(history) > 0 && now.Sub(history[0]) >= window {
		history = history[1:]
	}
	return history
}

func (g *Group) receive(result completion) {
	g.mu.Lock()
	child := &g.slots[result.index]
	if !child.active || child.generation != result.generation {
		g.mu.Unlock()
		panic("invalid private child completion")
	}
	child.active = false
	child.cancel = nil
	g.active--
	failed := result.err != nil && !result.canceled
	if result.panicked {
		g.panics++
	}
	if failed {
		g.failures++
	}
	if g.phase == Stopping {
		if failed {
			g.remember(result.err)
		}
		g.mu.Unlock()
		return
	}
	restart := child.policy == Permanent || (child.policy == Transient && failed)
	if !restart {
		if failed {
			g.remember(result.err)
		}
		g.mu.Unlock()
		return
	}
	now := time.Now()
	g.history = expire(g.history, now, g.options.Window)
	if len(g.history) >= g.options.Restarts {
		g.remember(ErrIntensity)
		g.mu.Unlock()
		g.RequestStop()
		return
	}
	g.history = append(g.history, now)
	g.restarts++
	child.pending = true
	child.restartAt = now.Add(g.options.RestartDelay)
	g.mu.Unlock()
}

func cleanupCall(cleanup func() error) (err error) {
	returned := false
	defer func() {
		if !returned {
			_ = recover()
			err = ErrCleanupPanicked
		}
	}()
	if cleanup != nil {
		err = boundedError(cleanup())
	}
	returned = true
	return
}

func (g *Group) manage() {
	for i := range g.slots {
		g.launch(i)
	}
	for {
		g.mu.Lock()
		pending := false
		var next time.Time
		for _, child := range g.slots {
			if child.pending {
				pending = true
				if next.IsZero() || child.restartAt.Before(next) {
					next = child.restartAt
				}
			}
		}
		if g.phase == Running && g.active == 0 && !pending {
			g.mu.Unlock()
			g.RequestStop()
			continue
		}
		if g.phase == Stopping && g.active == 0 {
			g.phase = Cleaning
			g.cleanupActive = true
			cleanup := g.cleanup
			g.cleanup = nil
			for i := range g.slots {
				g.slots[i].factory = nil
			}
			g.history = nil
			g.mu.Unlock()
			err := cleanupCall(cleanup)
			g.mu.Lock()
			if err != nil {
				g.remember(err)
			}
			g.cleanupActive = false
			g.phase = Complete
			g.mu.Unlock()
			close(g.done)
			return
		}
		g.mu.Unlock()
		var timer *time.Timer
		var tick <-chan time.Time
		if pending {
			timer = time.NewTimer(time.Until(next))
			tick = timer.C
		}
		select {
		case result := <-g.finished:
			g.receive(result)
		case <-g.wake:
		case <-tick:
			for i := range g.slots {
				g.mu.Lock()
				due := g.slots[i].pending && !time.Now().Before(g.slots[i].restartAt)
				g.mu.Unlock()
				if due {
					g.launch(i)
				}
			}
		}
		if timer != nil {
			timer.Stop()
		}
	}
}

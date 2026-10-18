#!/usr/bin/env python3
"""Exact closed observation source bind; errors never project file contents/paths."""
import hashlib
from pathlib import Path

ORIGINAL = {'.github/workflows/ci.yml': '3f3772a9506ea3044f3a66ea7985406dbc55fcea02850e1e78b47879f7b3890b',
 'compiler/Cargo.toml': '4d94633b9fb5670493dc700a70a297c7911844608c8c74de11004db4ff144b0f',
 'compiler/src/editor.rs': '44ec525acdfcd00a7690ff02129703ec8060d696a790324190f59627e9e14e76',
 'compiler/src/emit.rs': '5c87ce9f41637297dac3166d01b5790497c39de7669cd5bcce966568edd4f5ad',
 'compiler/src/lib.rs': 'bcc62519b0759b1521d682c084b2f41cf8d0ba2514cfce8df9bfb79457c07f9b',
 'compiler/src/main.rs': '97f46de717c56d50cb69418106adb28bbce81ae9a8b48c3480ea0dc0a2e9d1d0',
 'compiler/src/symbols.rs': '28d1a145792f42bdcbc77a98b25fedbf4a0d42ec8aee3f7f03735ae0ef553244',
 'editors/vscode-nagi/package.json': 'cb6c01551482fa5592d6d82b7c090c92084bbc3fa2d219e929d82414fd49ca16',
 'editors/vscode-nagi/src/compiler.js': '7e01246e33c70e08cb54ff962616b81d7e98d8a2598ab46abde4a193d0011090',
 'editors/vscode-nagi/test/actor-fixtures.js': '4b791baf04cd62f4b80504f0ffe42cc5769cf94ccf20a59c8ebe5f5c86d6dc3f',
 'editors/vscode-nagi/test/actor-navigation.test.js': '6c6a9b4da92dbf6dfffcd77b98405752256ca08253b9838b3f2c17dffbe4c1ef',
 'editors/vscode-nagi/test/compiler.test.js': 'a268b5b40fe94df9926cac219a27332153f2f67955d052871d191e409d865523',
 'editors/vscode-nagi/test/stdlib-fixtures.js': 'd4b95fd330c5479178c07e6e8f129f2b3e9846ad392ccf580e1bb82573a0d8a9',
 'editors/vscode-nagi/test/stdlib-navigation.test.js': '242d7b78d998045872f48dcb50e494a8914f9b7b780a6e2d56a9fe283e8f5f74',
 'scripts/test_graph_html_viewport.mjs': 'fee98ec8a14a035f60a713a2d54044c51927c2f5e3412fc87544b3c770ea0ed9'}

BOUND = {'.github/workflows/ci.yml': '3f3772a9506ea3044f3a66ea7985406dbc55fcea02850e1e78b47879f7b3890b',
 '.github/workflows/editor-stage-observation.yml': 'b1a5ada380c0d3bfd9326276fcde12edc3fc70ee7b6135fa136fcb0a3caa4720',
 'compiler/Cargo.toml': '45f764607b007dacdbb8b2cdb3c7379a512fdab70d63b2a4876c072fde7ef236',
 'compiler/src/editor.rs': '44ec525acdfcd00a7690ff02129703ec8060d696a790324190f59627e9e14e76',
 'compiler/src/editor_stage_diag.rs': 'dd9c89eb6a19048b1c925ec4d909c0153c4856ffe31727d7dbc56e2e699fcf8c',
 'compiler/src/emit.rs': '7b08d1d8afab3b05a0bebce9e3ec6d54fb32751e9bfac7d83d45fb81d51aebe9',
 'compiler/src/lib.rs': '45928732ffce44bf373d4538c6498623828e86fa9e615a1e34d0ef38c924ea5f',
 'compiler/src/main.rs': '97f46de717c56d50cb69418106adb28bbce81ae9a8b48c3480ea0dc0a2e9d1d0',
 'compiler/src/symbols.rs': 'f2ae3540d262b97f9f87ef029e571ceaf979de8411784de61802cd7b52e6950a',
 'editors/vscode-nagi/package.json': 'cb6c01551482fa5592d6d82b7c090c92084bbc3fa2d219e929d82414fd49ca16',
 'editors/vscode-nagi/src/compiler.js': '7e01246e33c70e08cb54ff962616b81d7e98d8a2598ab46abde4a193d0011090',
 'editors/vscode-nagi/test/actor-fixtures.js': '4b791baf04cd62f4b80504f0ffe42cc5769cf94ccf20a59c8ebe5f5c86d6dc3f',
 'editors/vscode-nagi/test/actor-navigation.test.js': 'f921041798b90c8ed50efea8858606c19cdb897019309556f2b8a4a435a3e248',
 'editors/vscode-nagi/test/compiler.test.js': 'a268b5b40fe94df9926cac219a27332153f2f67955d052871d191e409d865523',
 'editors/vscode-nagi/test/editor-stage-observation.js': '91ca2af2c32da42cc30d19a5c4c0712de5bb1965b8c5799d897cd610a5faa11f',
 'editors/vscode-nagi/test/stdlib-fixtures.js': 'd4b95fd330c5479178c07e6e8f129f2b3e9846ad392ccf580e1bb82573a0d8a9',
 'editors/vscode-nagi/test/stdlib-navigation.test.js': 'f09a42f00fd126281fc709c8321f341e886ee98e37671059fa8ceb720996487f',
 'scripts/ci/editor_stage_measurement.cjs': '6350f33b5aabe5bf8204162bcb6c4775d27c8abe31e0885bcee6abd9cc46f1a9',
 'scripts/ci/editor_stage_reporter.cjs': '405f7ebc558307d8c6fc442853ac6c5b120656e27591857d3de3d7d621f9f4ac',
 'scripts/test_graph_html_viewport.mjs': 'fee98ec8a14a035f60a713a2d54044c51927c2f5e3412fc87544b3c770ea0ed9',
 'scripts/tests/test_editor_stage_observation.cjs': '47102aef41b8196b9e91316d604086161838484efd9b67c8a4509dc59d724729',
 'scripts/tests/test_editor_stage_observation_source.py': 'ce13854fdf8440e2f0af0a33c177ddf38efe24c14c2f078cb28b651c664a18b2'}

INVERSE = {'compiler/Cargo.toml': [('path = "../fuzz/smoke.rs"\n'
                          '\n'
                          '[lints.rust]\n'
                          'unexpected_cfgs = { level = "warn", check-cfg = [\'cfg(nagi_sf01_diagnostic)\', '
                          "'cfg(nagi_sf01_diag_oracle)', 'cfg(nagi_editor_stage_diag)'] }\n",
                          'path = "../fuzz/smoke.rs"\n'
                          '\n'
                          '[lints.rust]\n'
                          'unexpected_cfgs = { level = "warn", check-cfg = [\'cfg(nagi_sf01_diagnostic)\', '
                          "'cfg(nagi_sf01_diag_oracle)'] }\n")],
 'compiler/src/emit.rs': [('}\n'
                           '\n'
                           'pub fn cli(args: Vec<String>) -> Result<(), String> {\n'
                           '    #[cfg(nagi_editor_stage_diag)]\n'
                           '    let _editor_stage = crate::editor_stage_diag::begin(&args);\n'
                           '    if crate::sql_check::worker(&args)? {\n'
                           '        return Ok(());\n'
                           '    }\n',
                           '}\n'
                           '\n'
                           'pub fn cli(args: Vec<String>) -> Result<(), String> {\n'
                           '    if crate::sql_check::worker(&args)? {\n'
                           '        return Ok(());\n'
                           '    }\n'),
                          ('    } else {\n'
                           '        (crate::project::resolve(&args, &cwd)?, None)\n'
                           '    };\n'
                           '    #[cfg(nagi_editor_stage_diag)]\n'
                           '    crate::editor_stage_diag::mark(1);\n'
                           '    let project_manifest = options.manifest_path;\n'
                           '    let cmd = options.command.as_str();\n'
                           '    let path = options.source;\n',
                           '    } else {\n'
                           '        (crate::project::resolve(&args, &cwd)?, None)\n'
                           '    };\n'
                           '    let project_manifest = options.manifest_path;\n'
                           '    let cmd = options.command.as_str();\n'
                           '    let path = options.source;\n'),
                          ('    } else {\n'
                           '        std::collections::HashMap::new()\n'
                           '    };\n'
                           '    #[cfg(nagi_editor_stage_diag)]\n'
                           '    crate::editor_stage_diag::mark(2);\n'
                           '    let mut sources = crate::source::load_with_overlays(&path, high, '
                           '&overlays)?;\n'
                           '    #[cfg(nagi_editor_stage_diag)]\n'
                           '    crate::editor_stage_diag::mark(3);\n'
                           '    let mut p = std::mem::take(&mut sources.program);\n'
                           '    let native = options.native;\n'
                           '    let out = options.out;\n',
                           '    } else {\n'
                           '        std::collections::HashMap::new()\n'
                           '    };\n'
                           '    let mut sources = crate::source::load_with_overlays(&path, high, '
                           '&overlays)?;\n'
                           '    let mut p = std::mem::take(&mut sources.program);\n'
                           '    let native = options.native;\n'
                           '    let out = options.out;\n'),
                          ('            .map_err(|e| sources.diagnostic(&e))?;\n'
                           '    }\n'
                           '    crate::modules::rebind_native(&mut p, &mut all).map_err(|e| '
                           'sources.diagnostic(&e))?;\n'
                           '    #[cfg(nagi_editor_stage_diag)]\n'
                           '    crate::editor_stage_diag::mark(4);\n'
                           '    if cmd == "symbols" {\n'
                           '        #[cfg(not(nagi_editor_stage_diag))]\n'
                           '        println!("{}", crate::symbols::index(&sources, &[&p, &all])?);\n'
                           '        #[cfg(nagi_editor_stage_diag)]\n'
                           '        {\n'
                           '            crate::editor_stage_diag::mark(5);\n'
                           '            let index = crate::symbols::index(&sources, &[&p, &all])?;\n'
                           '            crate::editor_stage_diag::mark(8);\n'
                           '            crate::editor_stage_diag::mark(9);\n'
                           '            println!("{}", index);\n'
                           '            crate::editor_stage_diag::mark(10);\n'
                           '        }\n'
                           '        return Ok(());\n'
                           '    }\n'
                           '    // 手書きLowの通常関数はHighの名前解決にも使う。置換本体はLowの統合時に検査する。\n',
                           '            .map_err(|e| sources.diagnostic(&e))?;\n'
                           '    }\n'
                           '    crate::modules::rebind_native(&mut p, &mut all).map_err(|e| '
                           'sources.diagnostic(&e))?;\n'
                           '    if cmd == "symbols" {\n'
                           '        println!("{}", crate::symbols::index(&sources, &[&p, &all])?);\n'
                           '        return Ok(());\n'
                           '    }\n'
                           '    // 手書きLowの通常関数はHighの名前解決にも使う。置換本体はLowの統合時に検査する。\n')],
 'compiler/src/lib.rs': [('mod constant_eval;\n'
                          'pub mod diagnostics;\n'
                          'pub(crate) mod editor;\n'
                          '#[cfg(nagi_editor_stage_diag)]\n'
                          'mod editor_stage_diag;\n'
                          'pub mod emit;\n'
                          'mod generation;\n'
                          'pub mod graph;\n',
                          'mod constant_eval;\n'
                          'pub mod diagnostics;\n'
                          'pub(crate) mod editor;\n'
                          'pub mod emit;\n'
                          'mod generation;\n'
                          'pub mod graph;\n')],
 'compiler/src/symbols.rs': [('        dest.modules.merge_native(p.modules.clone())?;\n'
                              '    }\n'
                              '    let owned_types = analyze\n'
                              '        .then(|| {\n'
                              '            #[cfg(nagi_editor_stage_diag)]\n'
                              '            crate::editor_stage_diag::mark(6);\n'
                              '            let types = crate::check::editor_types(&primary, &native);\n'
                              '            #[cfg(nagi_editor_stage_diag)]\n'
                              '            crate::editor_stage_diag::mark(7);\n'
                              '            types\n'
                              '        })\n'
                              '        .flatten();\n'
                              '    let typed = typed.or(owned_types.as_ref());\n'
                              '    let mut types = Types {\n',
                              '        dest.modules.merge_native(p.modules.clone())?;\n'
                              '    }\n'
                              '    let owned_types = analyze\n'
                              '        .then(|| crate::check::editor_types(&primary, &native))\n'
                              '        .flatten();\n'
                              '    let typed = typed.or(owned_types.as_ref());\n'
                              '    let mut types = Types {\n')],
 'editors/vscode-nagi/test/actor-navigation.test.js': [("const fs = require('node:fs');\n"
                                                        "const path = require('node:path');\n"
                                                        "const compiler = require('../src/compiler');\n"
                                                        'const observation = '
                                                        "require('./editor-stage-observation');\n"
                                                        "const features = require('../src/features');\n"
                                                        "const { fixtures } = require('./actor-fixtures');\n"
                                                        "const root = path.resolve(__dirname, '../../..');\n",
                                                        "const fs = require('node:fs');\n"
                                                        "const path = require('node:path');\n"
                                                        "const compiler = require('../src/compiler');\n"
                                                        "const features = require('../src/features');\n"
                                                        "const { fixtures } = require('./actor-fixtures');\n"
                                                        'const root = path.resolve(__dirname, '
                                                        "'../../..');\n"),
                                                       ('  fs.writeFileSync(path.join(folder, '
                                                        '`main.${suffix}`), text);\n'
                                                        '  return folder;\n'
                                                        '}\n'
                                                        'async function symbols(folder, file, overlays = [], '
                                                        'observationId) {\n'
                                                        '  const result = await new Promise(resolve => '
                                                        'observation.runCheck(observationId, compiler, '
                                                        "compiler.compilerPath('', root, root),\n"
                                                        "    ['symbols', file, '--editor-input'], folder, "
                                                        '10000, resolve, 16 * 1024 * 1024,\n'
                                                        '    JSON.stringify({ files: overlays })));\n'
                                                        '  assert.equal(result.error, null, '
                                                        'result.output);\n',
                                                        '  fs.writeFileSync(path.join(folder, '
                                                        '`main.${suffix}`), text);\n'
                                                        '  return folder;\n'
                                                        '}\n'
                                                        'async function symbols(folder, file, overlays = []) '
                                                        '{\n'
                                                        '  const result = await new Promise(resolve => '
                                                        "compiler.runCheck(compiler.compilerPath('', root, "
                                                        'root),\n'
                                                        "    ['symbols', file, '--editor-input'], folder, "
                                                        '10000, resolve, 16 * 1024 * 1024,\n'
                                                        '    JSON.stringify({ files: overlays })));\n'
                                                        '  assert.equal(result.error, null, '
                                                        'result.output);\n'),
                                                       ('for (const [suffix, text] of fixtures) {\n'
                                                        '  test(`real ${suffix} actor symbols separate '
                                                        'modules and preserve three-parameter handles`, '
                                                        'async t => {\n'
                                                        '    const folder = fixture(t, suffix, text), file = '
                                                        'path.join(folder, `main.${suffix}`), source = { '
                                                        'file };\n'
                                                        '    const index = await symbols(folder, '
                                                        "`main.${suffix}`, [], suffix === 'low' ? "
                                                        "'actor_low' : undefined);\n"
                                                        '    '
                                                        'assert.deepEqual(index.standard_sources.map(item => '
                                                        'item.file).sort(), [actorSource, '
                                                        'httpSource].sort());\n'
                                                        '    const actor = index.definitions.find(item => '
                                                        'item.id?.module === actorSource && item.name === '
                                                        "'Actor');\n"
                                                        '    const turn = index.definitions.find(item => '
                                                        'item.id?.module === actorSource && item.name === '
                                                        "'Turn');\n",
                                                        'for (const [suffix, text] of fixtures) {\n'
                                                        '  test(`real ${suffix} actor symbols separate '
                                                        'modules and preserve three-parameter handles`, '
                                                        'async t => {\n'
                                                        '    const folder = fixture(t, suffix, text), file = '
                                                        'path.join(folder, `main.${suffix}`), source = { '
                                                        'file };\n'
                                                        '    const index = await symbols(folder, '
                                                        '`main.${suffix}`);\n'
                                                        '    '
                                                        'assert.deepEqual(index.standard_sources.map(item => '
                                                        'item.file).sort(), [actorSource, '
                                                        'httpSource].sort());\n'
                                                        '    const actor = index.definitions.find(item => '
                                                        'item.id?.module === actorSource && item.name === '
                                                        "'Actor');\n"
                                                        '    const turn = index.definitions.find(item => '
                                                        'item.id?.module === actorSource && item.name === '
                                                        "'Turn');\n")],
 'editors/vscode-nagi/test/stdlib-navigation.test.js': [("const fs = require('node:fs');\n"
                                                         "const path = require('node:path');\n"
                                                         "const compiler = require('../src/compiler');\n"
                                                         'const observation = '
                                                         "require('./editor-stage-observation');\n"
                                                         "const features = require('../src/features');\n"
                                                         'const { fixtures } = '
                                                         "require('./stdlib-fixtures');\n"
                                                         'const root = path.resolve(__dirname, '
                                                         "'../../..');\n",
                                                         "const fs = require('node:fs');\n"
                                                         "const path = require('node:path');\n"
                                                         "const compiler = require('../src/compiler');\n"
                                                         "const features = require('../src/features');\n"
                                                         'const { fixtures } = '
                                                         "require('./stdlib-fixtures');\n"
                                                         'const root = path.resolve(__dirname, '
                                                         "'../../..');\n"),
                                                        ('  fs.writeFileSync(path.join(folder, '
                                                         '`main.${suffix}`), text);\n'
                                                         '  return folder;\n'
                                                         '}\n'
                                                         'async function symbols(folder, file, overlays = '
                                                         '[], observationId) {\n'
                                                         '  const result = await new Promise(resolve => '
                                                         'observation.runCheck(observationId, compiler, '
                                                         "compiler.compilerPath('', root, root),\n"
                                                         "    ['symbols', file, '--editor-input'], folder, "
                                                         '10000, resolve, 16 * 1024 * 1024,\n'
                                                         '    JSON.stringify({ files: overlays })));\n'
                                                         '  assert.equal(result.error, null, '
                                                         'result.output);\n',
                                                         '  fs.writeFileSync(path.join(folder, '
                                                         '`main.${suffix}`), text);\n'
                                                         '  return folder;\n'
                                                         '}\n'
                                                         'async function symbols(folder, file, overlays = '
                                                         '[]) {\n'
                                                         '  const result = await new Promise(resolve => '
                                                         "compiler.runCheck(compiler.compilerPath('', root, "
                                                         'root),\n'
                                                         "    ['symbols', file, '--editor-input'], folder, "
                                                         '10000, resolve, 16 * 1024 * 1024,\n'
                                                         '    JSON.stringify({ files: overlays })));\n'
                                                         '  assert.equal(result.error, null, '
                                                         'result.output);\n'),
                                                        ('for (const [suffix, text] of fixtures) {\n'
                                                         '  test(`real ${suffix} standard symbols keep '
                                                         'resource identity, virtual sources and UTF-16 '
                                                         'targets`, async t => {\n'
                                                         '    const folder = fixture(t, suffix, text), file '
                                                         '= path.join(folder, `main.${suffix}`), source = { '
                                                         'file };\n'
                                                         '    const index = await symbols(folder, '
                                                         "`main.${suffix}`, [], suffix === 'nagi' ? "
                                                         "'stdlib_high' : undefined);\n"
                                                         '    const standard = '
                                                         'index.standard_sources.find(item => item.file === '
                                                         'virtual);\n'
                                                         '    assert.ok(standard);\n'
                                                         '    assert.equal(index.standard_modules[0].name, '
                                                         "'std.http.server');\n",
                                                         'for (const [suffix, text] of fixtures) {\n'
                                                         '  test(`real ${suffix} standard symbols keep '
                                                         'resource identity, virtual sources and UTF-16 '
                                                         'targets`, async t => {\n'
                                                         '    const folder = fixture(t, suffix, text), file '
                                                         '= path.join(folder, `main.${suffix}`), source = { '
                                                         'file };\n'
                                                         '    const index = await symbols(folder, '
                                                         '`main.${suffix}`);\n'
                                                         '    const standard = '
                                                         'index.standard_sources.find(item => item.file === '
                                                         'virtual);\n'
                                                         '    assert.ok(standard);\n'
                                                         '    assert.equal(index.standard_modules[0].name, '
                                                         "'std.http.server');\n")]}

def inverse_source(name, data):
    text = data.decode("utf-8")
    for added, old in INVERSE.get(name, []):
        if text.count(added) != 1:
            raise ValueError("inverse fragment mismatch")
        text = text.replace(added, old, 1)
    restored = text.encode("utf-8")
    if hashlib.sha256(restored).hexdigest() != ORIGINAL[name]:
        raise ValueError("original source mismatch")
    return restored


def verify(read):
    for name, digest in BOUND.items():
        data = read(name)
        if hashlib.sha256(data).hexdigest() != digest:
            raise ValueError("bound source mismatch")
        if name in ORIGINAL:
            inverse_source(name, data)
    return len(BOUND), len(INVERSE)


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[2]
    try:
        count, changed = verify(lambda name: (root / name).read_bytes())
        print('{"source_bind_pass":%d,"strict_inverse_pass":%d,"native_cases_executed":0}' % (count, changed))
    except Exception:
        print('{"source_bind_failed":true,"native_cases_executed":0}')
        raise SystemExit(1)

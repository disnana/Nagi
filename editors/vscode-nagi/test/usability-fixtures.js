'use strict';

// Small compiler-shaped snapshots: the catalog is available before any import,
// while checked bindings retain canonical IDs when names are rendered as aliases.
const eventConstants = ['STARTING', 'STARTED', 'READY', 'FAILED', 'PANICKED', 'RESTART_SCHEDULED',
  'STOPPED', 'INTENSITY_EXCEEDED', 'SHUTDOWN', 'LAGGED'];
const parameter = (name, type) => ({ name, type });
const readonly = (name, type) => ({ name, type, readonly: true });

function moduleFixture(name, text, rows) {
  const id = `stdlib:${name}`;
  const lines = text.split('\n');
  const location = (declaration, spelling) => {
    const line = lines.findIndex(text => text.startsWith(declaration));
    if (line < 0) throw new Error(`Missing fixture declaration: ${declaration}`);
    return { file: id, line: line + 1, column: lines[line].indexOf(spelling) + 1, length: spelling.length };
  };
  const members = rows.map(row => ({ parameters: [], fields: [], typeParameters: [], asynchronous: false,
    ...row, id: { module: id, kind: row.kind === 'resource' ? 'Resource' : 'Function', name: row.name },
    location: location(`${row.kind === 'resource' ? 'resource' : 'def'} ${row.name}`, row.name),
    ...(row.kind === 'resource' ? { constants: (row.constantNames || []).map(name => ({ name, kind: 'constant',
      signature: `${row.name}.${name}: ${row.name}`, parameters: [], return_type: row.name,
      location: location(`    ${name}: ${row.name}`, name) })) } : {}),
  }));
  for (const member of members) delete member.constantNames;
  return { name, id, members, source: { file: id, text } };
}

const result = moduleFixture('std.result',
  '# Compiler-provided standard library.\n' +
  'def map_error(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]\n', [
    { name: 'map_error', kind: 'function', signature: 'def map_error(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]',
      parameters: [parameter('value', 'Result[T, E]'), parameter('mapper', 'fn[E, F]')], return_type: 'Result[T, F]' },
  ]);

const actor = moduleFixture('std.actor', `# Compiler-provided standard library.
resource Supervisor[C]
resource Control
resource RestartPolicy
    TEMPORARY: RestartPolicy
    TRANSIENT: RestartPolicy
    PERMANENT: RestartPolicy
resource TaskReady
resource WaitKind
    TIMEOUT: WaitKind
    INVALID_TIMEOUT: WaitKind
resource WaitError
    kind: WaitKind
    message: view[str]
resource EventKind
${eventConstants.map(name => `    ${name}: EventKind`).join('\n')}
resource Event
def task_with_ready(group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], TaskReady, Future[Result[unit, Error]]], policy: RestartPolicy) -> Result[unit, Error]
def mark_ready(signal: view[TaskReady]) -> Result[unit, Error]
def next_event_timeout(control: view[Control], timeout_ms: i64) -> Future[Result[Option[Event], WaitError]]
`, [
  { name: 'Supervisor', kind: 'resource', signature: 'resource Supervisor[C]', typeParameters: ['C'] },
  { name: 'Control', kind: 'resource', signature: 'resource Control' },
  { name: 'RestartPolicy', kind: 'resource', signature: 'resource RestartPolicy', constantNames: ['TEMPORARY', 'TRANSIENT', 'PERMANENT'] },
  { name: 'TaskReady', kind: 'resource', signature: 'resource TaskReady' },
  { name: 'WaitKind', kind: 'resource', signature: 'resource WaitKind', constantNames: ['TIMEOUT', 'INVALID_TIMEOUT'] },
  { name: 'WaitError', kind: 'resource', signature: 'resource WaitError\n    kind: WaitKind (read-only)\n    message: view[str] (read-only)',
    fields: [readonly('kind', 'WaitKind'), readonly('message', 'view[str]')] },
  { name: 'EventKind', kind: 'resource', signature: 'resource EventKind', constantNames: eventConstants },
  { name: 'Event', kind: 'resource', signature: 'resource Event' },
  { name: 'task_with_ready', kind: 'function',
    signature: 'def task_with_ready(group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], TaskReady, Future[Result[unit, Error]]], policy: RestartPolicy) -> Result[unit, Error]',
    parameters: [parameter('group', 'view[Supervisor[C]]'), parameter('name', 'view[str]'),
      parameter('factory', 'fn[shared[C], TaskReady, Future[Result[unit, Error]]]'), parameter('policy', 'RestartPolicy')], return_type: 'Result[unit, Error]' },
  { name: 'mark_ready', kind: 'function', signature: 'def mark_ready(signal: view[TaskReady]) -> Result[unit, Error]',
    parameters: [parameter('signal', 'view[TaskReady]')], return_type: 'Result[unit, Error]' },
  { name: 'next_event_timeout', kind: 'function', asynchronous: true,
    signature: 'def next_event_timeout(control: view[Control], timeout_ms: i64) -> Future[Result[Option[Event], WaitError]]',
    parameters: [parameter('control', 'view[Control]'), parameter('timeout_ms', 'i64')], return_type: 'Future[Result[Option[Event], WaitError]]' },
]);

const http = moduleFixture('std.http.server', `# Compiler-provided standard library.
resource Request
def is_json_content_type(request: view[Request]) -> Result[bool, Error]
`, [
  { name: 'Request', kind: 'resource', signature: 'resource Request' },
  { name: 'is_json_content_type', kind: 'function', signature: 'def is_json_content_type(request: view[Request]) -> Result[bool, Error]',
    parameters: [parameter('request', 'view[Request]')], return_type: 'Result[bool, Error]' },
]);

const modules = [result, actor, http];
const standard_modules = modules.map(({ name, id, members }) => ({ name, id, members }));
const standard_sources = modules.map(module => module.source);
const catalogIndex = { format: 'nagi-symbols-v1', files: [], definitions: [], bindings: [], references: [],
  expressions: [], locals: [], standard_modules, standard_sources: [] };

function resourceAlias(member, name, fields = member.fields) {
  return { ...member, name, fields, signature: `resource ${name}${member.typeParameters.length ? `[${member.typeParameters.join(', ')}]` : ''}` +
    fields.map(field => `\n    ${field.name}: ${field.type} (read-only)`).join(''),
  constants: member.constants.map(constant => ({ ...constant, signature: `${name}.${constant.name}: ${name}`, return_type: name })) };
}

function checkedIndex(file) {
  const ready = resourceAlias(actor.members.find(item => item.name === 'TaskReady'), 'Ready');
  const waitKind = resourceAlias(actor.members.find(item => item.name === 'WaitKind'), 'WaitReason');
  const waitFields = [readonly('kind', 'WaitReason'), readonly('message', 'view[str]')];
  const waitError = resourceAlias(actor.members.find(item => item.name === 'WaitError'), 'WaitFailure', waitFields);
  const eventKind = resourceAlias(actor.members.find(item => item.name === 'EventKind'), 'EventType');
  const incoming = resourceAlias(http.members[0], 'Incoming');
  const mapError = { ...result.members[0], signature: 'def results.map_error(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]' };
  const task = { ...actor.members.find(item => item.name === 'task_with_ready'),
    signature: 'def actors.task_with_ready(group: view[actors.Supervisor[C]], name: view[str], factory: fn[shared[C], Ready, Future[Result[unit, Error]]], policy: actors.RestartPolicy) -> Result[unit, Error]',
    parameters: [parameter('group', 'view[actors.Supervisor[C]]'), parameter('name', 'view[str]'),
      parameter('factory', 'fn[shared[C], Ready, Future[Result[unit, Error]]]'), parameter('policy', 'actors.RestartPolicy')] };
  const mark = { ...actor.members.find(item => item.name === 'mark_ready'),
    signature: 'def actors.mark_ready(signal: view[Ready]) -> Result[unit, Error]', parameters: [parameter('signal', 'view[Ready]')] };
  const next = { ...actor.members.find(item => item.name === 'next_event_timeout'),
    signature: 'def actors.next_event_timeout(control: view[actors.Control], timeout_ms: i64) -> Future[Result[Option[actors.Event], WaitFailure]]',
    parameters: [parameter('control', 'view[actors.Control]'), parameter('timeout_ms', 'i64')],
    return_type: 'Future[Result[Option[actors.Event], WaitFailure]]' };
  const json = { ...http.members[1], signature: 'def http.is_json_content_type(request: view[Incoming]) -> Result[bool, Error]',
    parameters: [parameter('request', 'view[Incoming]')] };
  const actorMembers = actor.members.filter(item => item.kind === 'resource').map(member =>
    resourceAlias(member, `actors.${member.name}`, member.name === 'WaitError' ? waitFields : member.fields)).concat(task, mark, next);
  const bindings = [
    { file, name: 'results', kind: 'module', target: { file: result.id, line: 1, column: 1, length: 0 }, members: [mapError] },
    { file, name: 'actors', kind: 'module', target: { file: actor.id, line: 1, column: 1, length: 0 }, members: actorMembers.map(member => ({ ...member, name: member.id.name })) },
    { file, name: 'http', kind: 'module', target: { file: http.id, line: 1, column: 1, length: 0 }, members: [resourceAlias(http.members[0], 'http.Request'), json] },
    ...[ready, waitKind, waitError, eventKind, incoming].map(definition => ({ file, name: definition.name, kind: 'resource', definition })),
    { file, name: 'remap_error', kind: 'function', definition: { ...mapError, name: 'remap_error',
      signature: 'def remap_error(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]' } },
  ];
  return { ...catalogIndex, files: [file], definitions: modules.flatMap(module => module.members), bindings, standard_sources };
}

module.exports = { catalogIndex, checkedIndex, modules };

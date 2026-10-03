'use strict';

// Standard sources come from the configured compiler's registry, not the project.
const prefix = 'stdlib:';
const validModule = name => /^std(?:\.[a-z][a-z0-9_]*)+$/.test(name);

function sourceUri(file) {
  if (typeof file !== 'string' || !file.startsWith(prefix)) return undefined;
  const name = file.slice(prefix.length);
  return validModule(name) ? `nagi-stdlib:/${name.replaceAll('.', '/')}.nagi` : undefined;
}

function sourceFile(uri) {
  if (!uri || uri.scheme !== 'nagi-stdlib' || uri.authority || uri.query || uri.fragment) return undefined;
  const match = /^\/(std(?:\/[a-z][a-z0-9_]*)+)\.nagi$/.exec(uri.path);
  return match ? prefix + match[1].replaceAll('/', '.') : undefined;
}

function sources(index) {
  if (!Array.isArray(index?.standard_sources)) return [];
  return index.standard_sources.filter(item => sourceUri(item?.file) && typeof item.text === 'string' &&
    Buffer.byteLength(item.text) <= 1024 * 1024);
}

module.exports = { sourceUri, sourceFile, sources };

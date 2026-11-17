// The canonical document (#255): { format, source } with the author's exact source text.
// `paragraphs` mirrors vink-web `document::paragraphs`, the Reader's projection of a default
// document: runs of blank or whitespace-only lines separate blocks, a trailing `\r` is not shown.
export const defaultDocument = (source) => ({ format: 'default', source });

export function paragraphs(document) {
  const blocks = [];
  let lines = [];
  for (const raw of document.source.split('\n')) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
    if (line.trim() === '') {
      if (lines.length) blocks.push({ lines });
      lines = [];
    } else {
      lines.push(line);
    }
  }
  if (lines.length) blocks.push({ lines });
  return blocks;
}

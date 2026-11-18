// The canonical document (#255): { format, source } with the author's exact source text.
// `paragraphs` mirrors vink-web `document::paragraphs`, the Reader's projection of a default
// document: blank or whitespace-only lines separate blocks; CR, LF and CRLF delimit lines.
export const defaultDocument = (source) => ({ format: 'default', source });

export function paragraphs(document) {
  const blocks = [];
  let lines = [];
  for (const line of document.source.split(/\r\n|\r|\n/)) {
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

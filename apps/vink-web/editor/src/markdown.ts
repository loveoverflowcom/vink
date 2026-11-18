import { decodeHTMLStrict } from 'entities';
import { commonmarkLanguage } from '@codemirror/lang-markdown';
import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import { EditorSelection, EditorState, Range } from '@codemirror/state';
import { Decoration, DecorationSet, EditorView, ViewPlugin, ViewUpdate } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';

export interface OutlineItem {
  level: number;
  title: string;
  from: number;
  isTitle: boolean;
}

export interface OutlineSnapshot {
  status: 'ready' | 'partial';
  generation: number;
  items: OutlineItem[];
  title: string;
  activeFrom: number | null;
}

const HEADING_RE = /^(?:ATX|Setext)Heading([1-6])$/;
const SKIP_IN_HEADING = new Set([
  'HeaderMark',
  'EmphasisMark',
  'LinkMark',
  'URL',
  'CodeMark',
  'LinkTitle',
  'HTMLTag',
  'QuoteMark',
  'ListMark',
]);

function decodeHtmlEntity(raw: string): string {
  if (!raw.startsWith('&') || !raw.endsWith(';')) return raw;
  const inner = raw.slice(1, -1);
  if (inner.startsWith('#x') || inner.startsWith('#X')) {
    const code = Number.parseInt(inner.slice(2), 16);
    if (Number.isNaN(code) || code <= 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) {
      return '\uFFFD';
    }
    return String.fromCodePoint(code);
  }
  if (inner.startsWith('#')) {
    const code = Number.parseInt(inner.slice(1), 10);
    if (Number.isNaN(code) || code <= 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) {
      return '\uFFFD';
    }
    return String.fromCodePoint(code);
  }
  return decodeHTMLStrict(raw);
}

function headingLevel(name: string): number {
  const match = HEADING_RE.exec(name);
  return match ? Number(match[1]) : 0;
}

function selectionTouches(state: EditorState, from: number, to: number): boolean {
  return state.selection.ranges.some((range) => range.from <= to && range.to >= from);
}

function flattenHeadingText(doc: EditorState['doc'], node: SyntaxNode): string {
  const walk = (current: SyntaxNode): string => {
    if (current.name === 'Autolink') return doc.sliceString(current.from + 1, current.to - 1);
    if (current.name === 'InlineCode') {
      const marks = current.getChildren('CodeMark');
      if (marks.length === 2) {
        const code = doc.sliceString(marks[0].to, marks[1].from).replace(/\n/g, ' ');
        return code.startsWith(' ') && code.endsWith(' ') && /[^ ]/.test(code)
          ? code.slice(1, -1) : code;
      }
    }
    let pos = current.from;
    let out = '';
    for (let child = current.firstChild; child; child = child.nextSibling) {
      out += doc.sliceString(pos, child.from);
      if (child.name === 'Escape') {
        out += doc.sliceString(child.from + 1, child.to);
      } else if (child.name === 'Entity') {
        out += decodeHtmlEntity(doc.sliceString(child.from, child.to));
      } else if (child.name === 'HardBreak') {
        out += ' ';
      } else if (!SKIP_IN_HEADING.has(child.name)) {
        out += walk(child);
      }
      pos = child.to;
    }
    out += doc.sliceString(pos, current.to);
    return out;
  };
  return walk(node)
    .replace(/\t/g, ' ')
    .replace(/\r?\n|\r/g, ' ')
    .trim();
}

export function extractOutline(
  state: EditorState,
  generation: number,
  budgetMs = 20,
): OutlineSnapshot {
  const fullTree = ensureSyntaxTree(state, state.doc.length, budgetMs);
  const tree = fullTree ?? syntaxTree(state);
  const status: 'ready' | 'partial' = fullTree ? 'ready' : 'partial';
  const items: OutlineItem[] = [];
  let firstH1Seen = false;
  let title = '';

  tree.iterate({
    enter(nodeRef) {
      const level = headingLevel(nodeRef.name);
      if (!level) return;
      const rawTitle = flattenHeadingText(state.doc, nodeRef.node);
      const isTitle = level === 1 && !firstH1Seen;
      if (level === 1 && !firstH1Seen) {
        firstH1Seen = true;
        title = rawTitle.normalize('NFC');
      }
      items.push({
        level,
        title: rawTitle,
        from: nodeRef.from,
        isTitle,
      });
      return false;
    },
  });

  return {
    status,
    generation,
    items,
    title,
    activeFrom: activeHeadingFrom(items, state.selection.main.head),
  };
}

export function activeHeadingFrom(items: OutlineItem[], head: number): number | null {
  let active: number | null = null;
  for (const item of items) {
    if (item.from <= head) {
      active = item.from;
    } else {
      break;
    }
  }
  return active;
}

/** Extracts the first H1 title projection from raw Markdown source using CommonMark 0.31.2 core. */
export function extractFirstH1(source: string): string {
  const tree = commonmarkLanguage.parser.parse(source);
  const state = EditorState.create({ doc: source });
  let found = '';
  let done = false;
  tree.iterate({
    enter(nodeRef) {
      if (done) return false;
      if (headingLevel(nodeRef.name) === 1) {
        found = flattenHeadingText(state.doc, nodeRef.node).normalize('NFC');
        done = true;
        return false;
      }
      return undefined;
    },
  });
  return found;
}

const markerDeco = Decoration.mark({ class: 'cm-markdown-marker' });
const hideDeco = Decoration.replace({});
const boldDeco = Decoration.mark({ class: 'cm-writing-bold' });
const italicDeco = Decoration.mark({ class: 'cm-writing-italic' });
const codeDeco = Decoration.mark({ class: 'cm-writing-code' });
const linkDeco = Decoration.mark({ class: 'cm-writing-link' });

function buildLivePreviewDecorations(view: EditorView): DecorationSet {
  const decorations: Range<Decoration>[] = [];
  const { state } = view;
  const doc = state.doc;
  const tree = syntaxTree(state);
  const decoratedHeadingLines = new Set<number>();
  const visitedNodes = new Set<string>();

  const pushRange = (deco: Decoration, from: number, to: number) => {
    if (from < to) {
      decorations.push(deco.range(from, to));
    }
  };

  const pushLine = (cls: string, lineFrom: number) => {
    if (!decoratedHeadingLines.has(lineFrom)) {
      decoratedHeadingLines.add(lineFrom);
      decorations.push(Decoration.line({ class: cls }).range(lineFrom));
    }
  };

  for (const range of view.visibleRanges) {
    tree.iterate({
      from: range.from,
      to: range.to,
      enter(nodeRef) {
        const name = nodeRef.name;
        const key = `${name}:${nodeRef.from}:${nodeRef.to}`;
        if (visitedNodes.has(key)) return undefined;
        visitedNodes.add(key);
        const level = headingLevel(name);
        if (level) {
          const isAtx = name.startsWith('ATX');
          const marks = nodeRef.node.getChildren('HeaderMark');
          if (isAtx) {
            const line = doc.lineAt(nodeRef.from);
            pushLine(`cm-writing-h${level}`, line.from);
            const active = selectionTouches(state, line.from, line.to);
            const openingMark = marks[0];
            const hasContent =
              openingMark !== undefined &&
              doc
                .sliceString(openingMark.to, marks.length > 1 ? marks[1].from : line.to)
                .trim().length > 0;
            for (let idx = 0; idx < marks.length; idx++) {
              const mark = marks[idx];
              const markLine = doc.lineAt(mark.from);
              if (idx === 0) {
                const afterSpaces =
                  doc.sliceString(mark.to, markLine.to).match(/^[ \t]+/)?.[0].length ?? 0;
                const start = mark.from;
                if (active || !hasContent) {
                  pushRange(markerDeco, mark.from, mark.to);
                } else {
                  pushRange(hideDeco, start, mark.to + afterSpaces);
                }
              } else {
                const beforeSpaces =
                  doc.sliceString(markLine.from, mark.from).match(/[ \t]+$/)?.[0].length ?? 0;
                if (active) {
                  pushRange(markerDeco, mark.from, mark.to);
                } else {
                  pushRange(hideDeco, mark.from - beforeSpaces, mark.to);
                }
              }
            }
          } else {
            const underlineFrom = marks[0]?.from ?? nodeRef.to;
            const startLineNum = doc.lineAt(nodeRef.from).number;
            const lastTextLineNum = Math.max(
              startLineNum,
              doc.lineAt(Math.max(nodeRef.from, underlineFrom - 1)).number,
            );
            for (let ln = startLineNum; ln <= lastTextLineNum; ln++) {
              const line = doc.line(ln);
              pushLine(`cm-writing-h${level}`, line.from);
            }
            for (const mark of marks) {
              pushRange(markerDeco, mark.from, mark.to);
            }
          }
          return undefined;
        }

        if (name === 'StrongEmphasis' || name === 'Emphasis') {
          const active = selectionTouches(state, nodeRef.from, nodeRef.to);
          const marks = nodeRef.node.getChildren('EmphasisMark');
          const styleDeco = name === 'Emphasis' ? italicDeco : boldDeco;
          if (marks.length === 2) {
            if (active) {
              pushRange(markerDeco, marks[0].from, marks[0].to);
              pushRange(styleDeco, marks[0].to, marks[1].from);
              pushRange(markerDeco, marks[1].from, marks[1].to);
            } else {
              pushRange(hideDeco, marks[0].from, marks[0].to);
              pushRange(styleDeco, marks[0].to, marks[1].from);
              pushRange(hideDeco, marks[1].from, marks[1].to);
            }
          }
          return undefined;
        }

        if (name === 'InlineCode') {
          const active = selectionTouches(state, nodeRef.from, nodeRef.to);
          const marks = nodeRef.node.getChildren('CodeMark');
          if (marks.length === 2) {
            if (active) {
              pushRange(markerDeco, marks[0].from, marks[0].to);
              pushRange(codeDeco, marks[0].to, marks[1].from);
              pushRange(markerDeco, marks[1].from, marks[1].to);
            } else {
              pushRange(hideDeco, marks[0].from, marks[0].to);
              pushRange(codeDeco, marks[0].to, marks[1].from);
              pushRange(hideDeco, marks[1].from, marks[1].to);
            }
          }
          return undefined;
        }

        if (name === 'Link') {
          const active = selectionTouches(state, nodeRef.from, nodeRef.to);
          const marks = nodeRef.node.getChildren('LinkMark');
          if (marks.length >= 4) {
            if (active) {
              for (const mark of marks) {
                pushRange(markerDeco, mark.from, mark.to);
              }
              pushRange(linkDeco, marks[0].to, marks[1].from);
            } else {
              pushRange(hideDeco, nodeRef.from, marks[0].to);
              pushRange(linkDeco, marks[0].to, marks[1].from);
              pushRange(hideDeco, marks[1].from, nodeRef.to);
            }
          }
          return undefined;
        }

        if (name === 'QuoteMark') {
          const line = doc.lineAt(nodeRef.from);
          if (!decoratedHeadingLines.has(line.from)) {
            decorations.push(Decoration.line({ class: 'cm-writing-quote' }).range(line.from));
          }
          const active = selectionTouches(state, line.from, line.to);
          const trailingSpace = doc.sliceString(nodeRef.to, nodeRef.to + 1) === ' ' ? 1 : 0;
          if (active) {
            pushRange(markerDeco, nodeRef.from, nodeRef.to);
          } else {
            pushRange(hideDeco, nodeRef.from, nodeRef.to + trailingSpace);
          }
          return undefined;
        }

        if (name === 'CodeMark' || name === 'ListMark') {
          pushRange(markerDeco, nodeRef.from, nodeRef.to);
          return undefined;
        }

        return undefined;
      },
    });
  }

  return Decoration.set(decorations, true);
}

export const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildLivePreviewDecorations(view);
    }

    update(update: ViewUpdate) {
      if (update.view.composing || update.view.compositionStarted) {
        this.decorations = this.decorations.map(update.changes);
        return;
      }
      if (
        update.docChanged ||
        update.selectionSet ||
        update.viewportChanged ||
        update.geometryChanged ||
        syntaxTree(update.startState) !== syntaxTree(update.state)
      ) {
        this.decorations = buildLivePreviewDecorations(update.view);
      }
    }
  },
  {
    decorations: (value) => value.decorations,
  },
);

export const markdownExtension = [commonmarkLanguage, livePreviewPlugin];

export function applyHeading(view: EditorView, level: number): boolean {
  if (view.composing || view.compositionStarted || view.state.readOnly) {
    return false;
  }
  const head = view.state.selection.main.head;
  const line = view.state.doc.lineAt(head);
  const existing = line.text.match(/^[ \t]{0,3}#{1,6}(?:[ \t]+|$)/)?.[0].length ?? 0;
  const prefix = level >= 1 && level <= 6 ? `${'#'.repeat(level)} ` : '';
  const nextAnchor = Math.max(line.from + prefix.length, head - existing + prefix.length);
  view.dispatch({
    changes: { from: line.from, to: line.from + existing, insert: prefix },
    selection: EditorSelection.cursor(Math.min(nextAnchor, line.from + prefix.length + (line.length - existing))),
    userEvent: 'input.format',
  });
  view.focus();
  return true;
}

export function applyInlineFormat(view: EditorView, kind: 'bold' | 'italic' | 'link'): boolean {
  if (view.composing || view.compositionStarted || view.state.readOnly) {
    return false;
  }
  const { from, to } = view.state.selection.main;
  const selected = view.state.sliceDoc(from, to);

  if (kind === 'link') {
    const wrapped = `[${selected}]()`;
    const caret = selected.length > 0 ? from + wrapped.length - 1 : from + 1;
    view.dispatch({
      changes: { from, to, insert: wrapped },
      selection: EditorSelection.cursor(caret),
      userEvent: 'input.format',
    });
    view.focus();
    return true;
  }

  const marker = kind === 'bold' ? '**' : '*';
  const before = from >= marker.length ? view.state.sliceDoc(from - marker.length, from) : '';
  const after = view.state.sliceDoc(to, to + marker.length);
  if (before === marker && after === marker && from !== to) {
    view.dispatch({
      changes: [
        { from: from - marker.length, to: from, insert: '' },
        { from: to, to: to + marker.length, insert: '' },
      ],
      selection: EditorSelection.single(from - marker.length, to - marker.length),
      userEvent: 'input.format',
    });
    view.focus();
    return true;
  }

  const wrapped = `${marker}${selected}${marker}`;
  const innerFrom = from + marker.length;
  const innerTo = innerFrom + selected.length;
  view.dispatch({
    changes: { from, to, insert: wrapped },
    selection: EditorSelection.single(innerFrom, innerTo),
    userEvent: 'input.format',
  });
  view.focus();
  return true;
}

export function jumpToHeading(
  view: EditorView,
  from: number,
  expectedGeneration: number,
  currentGeneration: number,
): boolean {
  if (
    expectedGeneration !== currentGeneration ||
    view.composing ||
    view.compositionStarted ||
    from < 0 ||
    from > view.state.doc.length
  ) {
    return false;
  }
  const line = view.state.doc.lineAt(from);
  view.dispatch({
    selection: EditorSelection.cursor(line.to),
    effects: EditorView.scrollIntoView(from, { y: 'center' }),
  });
  view.focus();
  return true;
}

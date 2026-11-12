/**
 * Two-way wiring between a DS toolbar and an nldd-text-editor, lifted from the
 * design system's text-editor "Mixed" story so the werkdocument editor gets the
 * exact same toolbar. The controls drive the editor's headless command API; the
 * editor's `state-change` event drives the controls (active formats,
 * disabled-in-code, indent/undo/redo/clipboard enablement, and the overflow-menu
 * mirrors). Framework-agnostic: it operates on the DOM, so the controls stay
 * uncontrolled and `onToolbarState` sets their properties directly.
 *
 * The toolbar lives in the page footer and the editor in the page body, so both
 * are resolved via their common `nldd-page` ancestor (not a single wrapper).
 */
import type { NLDDButton } from '@nldd/design-system/button';
import type { NLDDButtonBar } from '@nldd/design-system/button-bar';
import type { NLDDIconButton } from '@nldd/design-system/icon-button';
import type { NLDDMenu, NLDDMenuItem } from '@nldd/design-system/menu';
import type { NLDDSegmentedControl } from '@nldd/design-system/segmented-control';
import type {
	HeadingLevel,
	NLDDTextEditor,
	TextEditorActiveFormats,
	TextEditorState,
} from '@nldd/design-system/text-editor';
import type { NLDDToggleButton } from '@nldd/design-system/toggle-button';

/** The values of the toolbar's list segmented control. */
type ListControlValue = 'none' | 'bullet' | 'numbered';

// Handlers receive an event's target/currentTarget, which is always an element
// inside the nldd-page here.
const editorOf = (el: EventTarget | null): NLDDTextEditor | null | undefined =>
	(el as Element).closest('nldd-page')?.querySelector('nldd-text-editor');

export const reconcile = (
	el: EventTarget | null,
	keys: readonly (keyof TextEditorActiveFormats)[],
	values: readonly string[],
): void => {
	const editor = editorOf(el);
	if (!editor) return;
	const active = editor.getState().active;
	const desired = new Set(values);
	keys.forEach((key) => {
		if (desired.has(key) !== Boolean(active[key])) editor.runCommand(key);
	});
};

export const onListChange = (event: Event): void => {
	const value = (event as CustomEvent<{ value: ListControlValue }>).detail.value;
	editorOf(event.currentTarget)?.setList(value === 'numbered' ? 'ordered' : value);
};

const HEADING_LABELS = ['Paragraaf', 'Heading 1', 'Heading 2', 'Heading 3', 'Heading 4', 'Heading 5', 'Heading 6'];

export const onHeadingSelect = (event: Event): void => {
	const item = event.target as Element | null;
	if (item?.tagName !== 'NLDD-MENU-ITEM') return;
	const editor = editorOf(event.currentTarget);
	if (!editor) return;
	const value = item.getAttribute('value') ?? '0';
	const inCodeBlock = editor.getState().active.codeBlock;
	if (value === 'codeblock') {
		if (!inCodeBlock) editor.toggleCodeBlock(); // wrap; re-selecting it is a no-op
		return;
	}
	// A text style: a code block is its own block type, so step out of it first —
	// that's what makes "Paragraaf" double as the way out of a code block.
	if (inCodeBlock) editor.toggleCodeBlock();
	// The heading menu's values are 0-6.
	editor.setHeading(Number(value) as HeadingLevel);
};

export const onLink = (event: Event) => editorOf(event.currentTarget)?.toggleLink();
export const onIndent = (event: Event) => editorOf(event.currentTarget)?.indent();
export const onOutdent = (event: Event) => editorOf(event.currentTarget)?.outdent();
export const onUndo = (event: Event) => editorOf(event.currentTarget)?.undo();
export const onRedo = (event: Event) => editorOf(event.currentTarget)?.redo();
export const onCopy = (event: Event) => editorOf(event.currentTarget)?.copy();
export const onCut = (event: Event) => editorOf(event.currentTarget)?.cut();
export const onPaste = (event: Event) => editorOf(event.currentTarget)?.paste();
// Inline toggles (code, quote) run a named command straight through.
export const runCommand = (event: Event, name: string) => editorOf(event.currentTarget)?.runCommand(name);

// Run one overflow-menu action against its editor. Mirrors the inline handlers so
// an overflowed control behaves the same.
const runOverflowAction = (editor: NLDDTextEditor, action: string): void => {
	if (action.startsWith('heading:')) {
		const value = action.slice('heading:'.length);
		const inCodeBlock = editor.getState().active.codeBlock;
		if (value === 'codeblock') { if (!inCodeBlock) editor.toggleCodeBlock(); return; }
		if (inCodeBlock) editor.toggleCodeBlock();
		// heading:<n> items carry 0-6.
		editor.setHeading(Number(value) as HeadingLevel);
		return;
	}
	if (action.startsWith('list:')) {
		const value = action.slice('list:'.length) as ListControlValue;
		editor.setList(value === 'numbered' ? 'ordered' : value);
		return;
	}
	switch (action) {
		case 'copy': case 'cut': case 'paste':
		case 'undo': case 'redo':
		case 'indent': case 'outdent':
			editor[action](); break;
		case 'link': editor.toggleLink(); break;
		default: editor.runCommand(action); // bold, italic, strikethrough, inlineCode, quote
	}
};

// Overflow menu-items are cloned into a menu in document.body, so their @click
// listeners are lost — but the `select` event bubbles (composed). Catch it at the
// document, resolve the editor via the overflow menu's anchor (the overflow button
// lives in the toolbar's shadow root, itself inside the nldd-page), and run the
// item's `value` as an action. Scoped to the toolbar's own overflow menu so an
// inline menu (its own @select handler) isn't double-handled. Registered once.
let overflowListenerAttached = false;
export function attachOverflowSelectListener(): void {
	if (overflowListenerAttached) return;
	overflowListenerAttached = true;
	document.addEventListener('select', (event) => {
		const item = event.target as Element | null;
		if (item?.tagName !== 'NLDD-MENU-ITEM') return;
		// Find the toolbar's own overflow menu on the event path. It can sit several
		// levels up when the selected item lives in a submenu (e.g. "Tekststijl"
		// nests its options one nldd-menu deeper), so scan the whole path rather
		// than only the nearest ancestor menu.
		const menu = event.composedPath().find(
			(node): node is NLDDMenu => node instanceof Element
				&& node.tagName === 'NLDD-MENU'
				&& node.id.startsWith('nldd-toolbar-overflow-menu'),
		);
		if (!menu) return;
		const action = item.getAttribute('value');
		if (!action) return;
		// The root is the toolbar's ShadowRoot (with a host) or, detached, a node without one.
		const toolbar = (menu.anchorElement?.getRootNode() as { host?: Element } | undefined)?.host;
		const editor = toolbar?.closest('nldd-page')?.querySelector('nldd-text-editor');
		if (editor) runOverflowAction(editor, action);
	});
}

export const onToolbarState = (event: Event): void => {
	const detail = (event as CustomEvent<TextEditorState>).detail;
	const active = detail.active;
	// Registered on the nldd-page around the editor and its toolbar.
	const root = event.currentTarget as Element;
	// Multi-select segmented controls reflect via .values; single toggle buttons via .selected.
	const reflect = (group: string, keys: (keyof TextEditorActiveFormats)[]) => {
		const el = root.querySelector<NLDDSegmentedControl>(`[data-group="${group}"]`);
		if (el) el.values = keys.filter((key) => active[key]);
	};
	const reflectToggle = (group: string, key: keyof TextEditorActiveFormats) => {
		const el = root.querySelector<NLDDToggleButton>(`[data-group="${group}"]`);
		if (el) el.selected = Boolean(active[key]);
	};
	reflect('inline', ['bold', 'italic', 'strikethrough']);
	reflectToggle('code', 'inlineCode');
	reflectToggle('link', 'link');
	reflectToggle('quote', 'quote');
	const list = root.querySelector<NLDDSegmentedControl>('[data-group="list"]');
	if (list) list.value = active.orderedList ? 'numbered' : active.bulletList ? 'bullet' : 'none';

	// Formatting inside code is literal text, not markup: lock the inline formats in
	// any code, and the block formats inside a code block — only the code-block toggle
	// stays (to get back out).
	const inCode = active.inlineCode || active.codeBlock;
	const setDisabled = (group: string, cond: boolean) => {
		const el = root.querySelector<Element & { disabled: boolean }>(`[data-group="${group}"]`);
		if (el) el.disabled = cond;
	};
	setDisabled('inline', inCode);
	setDisabled('link', inCode);
	setDisabled('code', active.codeBlock);
	setDisabled('quote', active.codeBlock);
	setDisabled('list', active.codeBlock);

	// History buttons: enable undo/redo only when there's history in that direction,
	// and dim the whole bar (divider included) when neither applies.
	const historyBar = root.querySelector<NLDDButtonBar>('[data-group="history"]');
	if (historyBar) {
		const { canUndo, canRedo } = detail;
		historyBar.disabled = !canUndo && !canRedo;
		historyBar.updateComplete.then(() => {
			const [undoButton, redoButton] = historyBar.querySelectorAll<NLDDIconButton>('nldd-icon-button');
			if (undoButton) undoButton.disabled = !canUndo;
			if (redoButton) redoButton.disabled = !canRedo;
		});
	}

	// Clipboard bar: copy and cut need a selection; paste is always available.
	const clipboardBar = root.querySelector<NLDDButtonBar>('[data-group="clipboard"]');
	if (clipboardBar) {
		clipboardBar.updateComplete.then(() => {
			const [copyButton, cutButton] = clipboardBar.querySelectorAll<NLDDIconButton>('nldd-icon-button');
			if (copyButton) copyButton.disabled = detail.empty;
			if (cutButton) cutButton.disabled = detail.empty;
		});
	}

	// Indent buttons reflect what's possible: increase only with a parent to nest
	// under, decrease only when already nested. Disable the whole bar when neither.
	const indentBar = root.querySelector<NLDDButtonBar>('[data-group="indent"]');
	if (indentBar) {
		const { canIndent, canOutdent } = detail;
		indentBar.disabled = !canIndent && !canOutdent;
		indentBar.updateComplete.then(() => {
			const [increase, decrease] = indentBar.querySelectorAll<NLDDIconButton>('nldd-icon-button');
			if (increase) increase.disabled = !canIndent;
			if (decrease) decrease.disabled = !canOutdent;
		});
	}

	// The block-type menu also carries "Codeblok", and it stays enabled in a code
	// block — picking "Paragraaf" is how you get back out.
	const headingButton = root.querySelector<NLDDButton>('[data-group="heading"]');
	if (headingButton) headingButton.text = active.codeBlock ? 'Codeblok' : (HEADING_LABELS[active.heading] ?? 'Paragraaf');
	root.querySelectorAll<NLDDMenuItem>('#heading-menu nldd-menu-item').forEach((item) => {
		const value = item.getAttribute('value');
		item.selected = active.codeBlock ? value === 'codeblock' : Number(value) === active.heading;
	});

	// Mirror disabled + selected state onto the overflow fallbacks (light-DOM
	// originals and the live clones in the toolbar's overflow menu in document.body).
	// The formatting toolbar is the one carrying [data-group] controls (the page
	// also has a separate top toolbar with the title + Save button).
	const toolbar = root.querySelector('[data-group]')?.closest('nldd-toolbar');
	const overflowMenu = Array.from(document.querySelectorAll<NLDDMenu>('nldd-menu[id^="nldd-toolbar-overflow-menu"]'))
		.find((menu) => (menu.anchorElement?.getRootNode() as { host?: Element } | undefined)?.host === toolbar) ?? null;
	const { canUndo, canRedo, canIndent, canOutdent, empty } = detail;
	const disabledByValue = {
		bold: inCode, italic: inCode, strikethrough: inCode,
		inlineCode: active.codeBlock,
		link: inCode,
		quote: active.codeBlock,
		'list:none': active.codeBlock, 'list:bullet': active.codeBlock, 'list:numbered': active.codeBlock,
		indent: !canIndent, outdent: !canOutdent,
		copy: empty, cut: empty,
		undo: !canUndo, redo: !canRedo,
	};
	for (const [value, disabled] of Object.entries(disabledByValue)) {
		const selector = `nldd-menu-item[value="${value}"]`;
		root.querySelectorAll<NLDDMenuItem>(selector).forEach((item) => { item.disabled = disabled; });
		overflowMenu?.querySelectorAll<NLDDMenuItem>(selector).forEach((item) => { item.disabled = disabled; });
	}

	const noList = !active.bulletList && !active.orderedList;
	const selectedByValue = {
		bold: active.bold, italic: active.italic, strikethrough: active.strikethrough,
		inlineCode: active.inlineCode,
		link: active.link,
		quote: active.quote,
		'list:none': noList, 'list:bullet': active.bulletList, 'list:numbered': active.orderedList,
		'heading:0': !active.codeBlock && active.heading === 0,
		'heading:1': !active.codeBlock && active.heading === 1,
		'heading:2': !active.codeBlock && active.heading === 2,
		'heading:3': !active.codeBlock && active.heading === 3,
		'heading:4': !active.codeBlock && active.heading === 4,
		'heading:5': !active.codeBlock && active.heading === 5,
		'heading:6': !active.codeBlock && active.heading === 6,
		'heading:codeblock': active.codeBlock,
	};
	for (const [value, selected] of Object.entries(selectedByValue)) {
		const selector = `nldd-menu-item[value="${value}"]`;
		root.querySelectorAll<NLDDMenuItem>(selector).forEach((item) => { item.selected = selected; });
		overflowMenu?.querySelectorAll<NLDDMenuItem>(selector).forEach((item) => { item.selected = selected; });
	}
};

//! Markdown projection: parses a Markdown source with `pulldown-cmark` and
//! projects it as a vertical stack of styled blocks (headings, paragraphs,
//! lists, block quotes, code blocks, thematic breaks).
//!
//! Inline emphasis (bold/italic/code/strikethrough) is flattened into a single
//! styled label per inline run because picus labels carry one style per
//! label. When mixed emphasis is required within a paragraph, consecutive
//! same-style runs are merged and styled labels are laid out in a wrapping
//! flex row.
//!
//! Fenced code blocks are syntax-highlighted with `syntect` when a language
//! fence is present and a matching grammar is available; otherwise the raw
//! text is rendered in a monospace block.

use std::sync::Arc;

use bevy_ecs::{entity::Entity, prelude::Resource};
use crate::masonry_core::{
    layout::{Dim, Length},
    parley::{Alignment as ParleyAlignment, LineHeight, style::FontWeight},
    peniko::Color,
    properties::Padding,
};
use picus_view::style::Style as _;
use picus_view::view::{CrossAxisAlignment, FlexExt as _, flex_col, flex_row, label, sized_box};
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use syntect::highlighting::{Theme as SyntectTheme, ThemeSet};
use syntect::parsing::SyntaxSet;

use super::core::{ProjectionCtx, UiView};
use crate::UiMarkdown;
use crate::styling::{ResolvedStyle, apply_widget_style, font_stack_from_style, resolve_style};

/// Cached `syntect` syntax + theme state for code highlighting.
struct HighlightState {
    syntax_set: SyntaxSet,
    theme: SyntectTheme,
}

impl HighlightState {
    fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme: ThemeSet::load_defaults().themes["base16-ocean.dark"].clone(),
        }
    }
}

use std::sync::OnceLock;

static HIGHLIGHT: OnceLock<HighlightState> = OnceLock::new();

fn highlight_state() -> &'static HighlightState {
    HIGHLIGHT.get_or_init(HighlightState::new)
}

#[derive(Clone, Copy)]
struct MarkdownPalette {
    text: Color,
    muted: Color,
    code_bg: Color,
    code_text: Color,
    pre_bg: Color,
    quote_border: Color,
    link: Color,
    heading: Color,
    table_border: Color,
    table_header_bg: Color,
}

fn markdown_palette(style: &ResolvedStyle) -> Option<MarkdownPalette> {
    let text = style.colors.text?;
    let dark_text = color_luma(text) < 128;
    let link = if dark_text {
        Color::from_rgb8(0x09, 0x69, 0xDA)
    } else {
        Color::from_rgb8(0x58, 0xA6, 0xFF)
    };
    let code_text = if dark_text {
        Color::from_rgb8(0x82, 0x50, 0xDF)
    } else {
        Color::from_rgb8(0xF5, 0xC2, 0xE7)
    };

    Some(MarkdownPalette {
        text,
        muted: text.with_alpha(0.68),
        code_bg: text.with_alpha(0.10),
        code_text,
        pre_bg: text.with_alpha(0.07),
        quote_border: text.with_alpha(0.35),
        link,
        heading: text,
        table_border: text.with_alpha(0.24),
        table_header_bg: text.with_alpha(0.08),
    })
}

fn color_luma(color: Color) -> u8 {
    let rgba = color.to_rgba8();
    ((u32::from(rgba.r) * 299 + u32::from(rgba.g) * 587 + u32::from(rgba.b) * 114) / 1000) as u8
}

pub(crate) fn project_markdown(component: &UiMarkdown, ctx: ProjectionCtx<'_>) -> UiView {
    let base_style = resolve_style(ctx.world, ctx.entity);
    let Some(palette) = markdown_palette(&base_style) else {
        return Arc::new(apply_widget_style(
            flex_col(Vec::<picus_view::view::AnyFlexChild<(), ()>>::new()).width(Dim::Stretch),
            &base_style,
        ));
    };

    let blocks = parse_markdown_blocks(&component.source);

    Arc::new(apply_widget_style(
        flex_col(blocks_to_flex_children(blocks, &base_style, &palette))
            .width(Dim::Stretch)
            .gap(Length::px(8.0)),
        &base_style,
    ))
}

/// Resource caching parsed completed-prefix blocks for streaming markdown
/// documents, keyed by entity.
///
/// This avoids re-parsing the (growing) completed prefix every frame; only the
/// in-progress tail is re-parsed.
#[derive(Resource, Default)]
pub struct StreamingMarkdownParseCache {
    entries: std::collections::HashMap<Entity, CachedStreamingBlocks>,
}

struct CachedStreamingBlocks {
    completed_source: String,
    completed_blocks: Vec<MdBlock>,
}

impl StreamingMarkdownParseCache {
    /// Read the cached completed-prefix blocks for `entity` without mutation.
    ///
    /// Returns `None` when no cache entry exists (caller should parse from
    /// scratch).
    #[must_use]
    fn get_cached(&self, entity: Entity, completed_source: &str) -> Option<Vec<MdBlock>> {
        match self.entries.get(&entity) {
            Some(entry) if entry.completed_source == completed_source => {
                Some(entry.completed_blocks.clone())
            }
            _ => None,
        }
    }

    fn get_or_parse_completed(&mut self, entity: Entity, completed_source: &str) -> Vec<MdBlock> {
        match self.get_cached(entity, completed_source) {
            Some(blocks) => blocks,
            None => {
                let blocks = parse_markdown_blocks(completed_source);
                self.entries.insert(
                    entity,
                    CachedStreamingBlocks {
                        completed_source: completed_source.to_string(),
                        completed_blocks: blocks.clone(),
                    },
                );
                blocks
            }
        }
    }

    /// Remove a cache entry when its entity is despawned.
    pub fn evict(&mut self, entity: Entity) {
        self.entries.remove(&entity);
    }
}

/// `Update` system: refresh cached completed-prefix blocks for every
/// [`crate::UiStreamingMarkdown`] entity.
///
/// Runs before projection so the cache is populated when the projector reads
/// it (projectors only get `&World` and cannot mutate the cache).
pub fn update_streaming_markdown_cache(
    mut cache: bevy_ecs::prelude::ResMut<StreamingMarkdownParseCache>,
    streaming_query: bevy_ecs::prelude::Query<(
        bevy_ecs::prelude::Entity,
        &crate::UiStreamingMarkdown,
    )>,
) {
    for (entity, streaming) in streaming_query.iter() {
        let _ = cache.get_or_parse_completed(entity, streaming.completed_source());
    }
}

pub(crate) fn project_streaming_markdown(
    component: &crate::UiStreamingMarkdown,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    let base_style = resolve_style(ctx.world, ctx.entity);
    let Some(palette) = markdown_palette(&base_style) else {
        return Arc::new(apply_widget_style(
            flex_col(Vec::<picus_view::view::AnyFlexChild<(), ()>>::new()).width(Dim::Stretch),
            &base_style,
        ));
    };

    let completed_blocks = ctx
        .world
        .get_resource::<StreamingMarkdownParseCache>()
        .and_then(|cache| cache.get_cached(ctx.entity, component.completed_source()))
        .unwrap_or_else(|| parse_markdown_blocks(component.completed_source()));

    let tail_blocks = if component.in_progress_source().is_empty() {
        Vec::new()
    } else {
        parse_markdown_blocks(component.in_progress_source())
    };

    let mut all_blocks = completed_blocks;
    all_blocks.extend(tail_blocks);

    Arc::new(apply_widget_style(
        flex_col(blocks_to_flex_children(all_blocks, &base_style, &palette))
            .width(Dim::Stretch)
            .gap(Length::px(8.0)),
        &base_style,
    ))
}

fn blocks_to_flex_children(
    blocks: Vec<MdBlock>,
    base: &ResolvedStyle,
    palette: &MarkdownPalette,
) -> Vec<picus_view::view::AnyFlexChild<(), ()>> {
    use picus_view::view::FlexExt as _;
    blocks
        .into_iter()
        .map(|block| block_to_view(block, base, palette).into_any_flex())
        .collect::<Vec<_>>()
}

/// Remove streaming-markdown parse cache entries for despawned entities.
pub fn evict_streaming_markdown_cache(
    mut cache: bevy_ecs::prelude::ResMut<StreamingMarkdownParseCache>,
    streaming_query: bevy_ecs::prelude::Query<
        bevy_ecs::prelude::Entity,
        bevy_ecs::prelude::With<crate::UiStreamingMarkdown>,
    >,
) {
    let live: std::collections::HashSet<bevy_ecs::prelude::Entity> =
        streaming_query.iter().collect();
    cache.entries.retain(|entity, _| live.contains(entity));
}

/// A styled inline text run.
#[derive(Clone)]
struct InlineRun {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    strikethrough: bool,
    link: bool,
}

/// A single resolved markdown block.
#[derive(Clone)]
enum MdBlock {
    Heading {
        level: HeadingLevel,
        runs: Vec<InlineRun>,
    },
    Paragraph {
        runs: Vec<InlineRun>,
    },
    Code {
        language: Option<String>,
        code: String,
    },
    BlockQuote {
        children: Vec<MdBlock>,
    },
    UnorderedList {
        items: Vec<Vec<InlineRun>>,
    },
    OrderedList {
        start: u64,
        items: Vec<Vec<InlineRun>>,
    },
    Table {
        alignments: Vec<Alignment>,
        header: Vec<Vec<InlineRun>>,
        rows: Vec<Vec<Vec<InlineRun>>>,
    },
    ThematicBreak,
}

/// Parse markdown source into a list of blocks, resolving inline runs.
fn parse_markdown_blocks(source: &str) -> Vec<MdBlock> {
    let options = Options::ENABLE_GFM
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options);

    let mut blocks: Vec<MdBlock> = Vec::new();
    let mut inline_acc: Vec<InlineRun> = Vec::new();
    let mut emphasis_stack: Vec<EmphasisFlag> = Vec::new();
    let mut link_active = false;
    let mut current_heading: Option<HeadingLevel> = None;
    let mut code_block_lang: Option<Option<String>> = None;
    let mut code_block_acc = String::new();
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut list_items_acc: Vec<Vec<InlineRun>> = Vec::new();
    let mut quote_depth: usize = 0;
    let mut table: Option<MdTableDraft> = None;
    let mut table_cell_active = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if !table_cell_active {
                        inline_acc.clear();
                        emphasis_stack.clear();
                        link_active = false;
                    }
                }
                Tag::Heading { level, .. } => {
                    current_heading = Some(level);
                    inline_acc.clear();
                    emphasis_stack.clear();
                }
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(info) => {
                            Some(info.to_string()).filter(|s| !s.trim().is_empty())
                        }
                        pulldown_cmark::CodeBlockKind::Indented => None,
                    };
                    code_block_lang = Some(lang);
                    code_block_acc.clear();
                }
                Tag::BlockQuote(_) => {
                    quote_depth += 1;
                }
                Tag::List(start) => {
                    if !table_cell_active {
                        let kind = match start {
                            Some(n) => ListKind::Ordered(n),
                            None => ListKind::Unordered,
                        };
                        list_stack.push(kind);
                        list_items_acc.clear();
                    }
                }
                Tag::Item => {
                    if !table_cell_active {
                        inline_acc.clear();
                        emphasis_stack.clear();
                    }
                }
                Tag::Table(alignments) => {
                    table = Some(MdTableDraft::new(alignments));
                }
                Tag::TableHead => {
                    if let Some(table) = table.as_mut() {
                        table.in_header = true;
                    }
                }
                Tag::TableRow => {
                    if let Some(table) = table.as_mut() {
                        table.current_row = Some(Vec::new());
                    }
                }
                Tag::TableCell => {
                    table_cell_active = true;
                    inline_acc.clear();
                    emphasis_stack.clear();
                    link_active = false;
                }
                Tag::Emphasis => emphasis_stack.push(EmphasisFlag::Italic),
                Tag::Strong => emphasis_stack.push(EmphasisFlag::Bold),
                Tag::Strikethrough => emphasis_stack.push(EmphasisFlag::Strike),
                Tag::Link { .. } => {
                    link_active = true;
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Paragraph => {
                    if !table_cell_active {
                        let runs = std::mem::take(&mut inline_acc);
                        nest_into_quote(quote_depth, &mut blocks, MdBlock::Paragraph { runs });
                    }
                }
                TagEnd::Heading(_) => {
                    let runs = std::mem::take(&mut inline_acc);
                    let level = current_heading.take().unwrap_or(HeadingLevel::H1);
                    nest_into_quote(quote_depth, &mut blocks, MdBlock::Heading { level, runs });
                }
                TagEnd::CodeBlock => {
                    let lang = code_block_lang.take().flatten();
                    let code = std::mem::take(&mut code_block_acc);
                    nest_into_quote(
                        quote_depth,
                        &mut blocks,
                        MdBlock::Code {
                            language: lang,
                            code,
                        },
                    );
                }
                TagEnd::BlockQuote(_) => {
                    quote_depth = quote_depth.saturating_sub(1);
                }
                TagEnd::List(_) => {
                    if !table_cell_active {
                        let items = std::mem::take(&mut list_items_acc);
                        let kind = list_stack.pop().unwrap_or(ListKind::Unordered);
                        let block = match kind {
                            ListKind::Unordered => MdBlock::UnorderedList { items },
                            ListKind::Ordered(start) => MdBlock::OrderedList { start, items },
                        };
                        nest_into_quote(quote_depth, &mut blocks, block);
                    }
                }
                TagEnd::Item => {
                    if !table_cell_active {
                        let runs = std::mem::take(&mut inline_acc);
                        list_items_acc.push(runs);
                    }
                }
                TagEnd::TableCell => {
                    if let Some(table) = table.as_mut() {
                        table.push_cell(std::mem::take(&mut inline_acc));
                    }
                    table_cell_active = false;
                }
                TagEnd::TableRow => {
                    if let Some(table) = table.as_mut() {
                        table.finish_row();
                    }
                }
                TagEnd::TableHead => {
                    if let Some(table) = table.as_mut() {
                        if table.current_row.is_some() {
                            table.finish_row();
                        }
                        table.in_header = false;
                    }
                }
                TagEnd::Table => {
                    if let Some(table) = table.take() {
                        nest_into_quote(quote_depth, &mut blocks, table.into_block());
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    emphasis_stack.pop();
                }
                TagEnd::Link => {
                    link_active = false;
                }
                _ => {}
            },
            Event::Text(text) => {
                if code_block_lang.is_some() {
                    code_block_acc.push_str(&text);
                } else {
                    push_inline_run(&mut inline_acc, &emphasis_stack, link_active, &text);
                }
            }
            Event::Code(text) => {
                let mut code_stack = emphasis_stack.clone();
                code_stack.push(EmphasisFlag::Code);
                push_inline_run(&mut inline_acc, &code_stack, link_active, &text);
            }
            Event::SoftBreak => {
                push_inline_run(&mut inline_acc, &emphasis_stack, link_active, " ");
            }
            Event::HardBreak => {
                push_inline_run(&mut inline_acc, &emphasis_stack, link_active, "\n");
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "☑ " } else { "☐ " };
                push_inline_run(&mut inline_acc, &emphasis_stack, link_active, marker);
            }
            Event::Rule if !table_cell_active => {
                nest_into_quote(quote_depth, &mut blocks, MdBlock::ThematicBreak);
            }
            _ => {}
        }
    }

    blocks
}

#[derive(Clone, Copy)]
enum ListKind {
    Unordered,
    Ordered(u64),
}

struct MdTableDraft {
    alignments: Vec<Alignment>,
    header: Vec<Vec<InlineRun>>,
    rows: Vec<Vec<Vec<InlineRun>>>,
    current_row: Option<Vec<Vec<InlineRun>>>,
    in_header: bool,
}

impl MdTableDraft {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            header: Vec::new(),
            rows: Vec::new(),
            current_row: None,
            in_header: false,
        }
    }

    fn push_cell(&mut self, runs: Vec<InlineRun>) {
        self.current_row.get_or_insert_with(Vec::new).push(runs);
    }

    fn finish_row(&mut self) {
        let row = self.current_row.take().unwrap_or_default();
        if self.in_header {
            self.header = row;
        } else {
            self.rows.push(row);
        }
    }

    fn into_block(self) -> MdBlock {
        MdBlock::Table {
            alignments: self.alignments,
            header: self.header,
            rows: self.rows,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum EmphasisFlag {
    Bold,
    Italic,
    Strike,
    Code,
}

fn push_inline_run(acc: &mut Vec<InlineRun>, stack: &[EmphasisFlag], link: bool, text: &str) {
    let bold = stack.contains(&EmphasisFlag::Bold);
    let italic = stack.contains(&EmphasisFlag::Italic);
    let code = stack.contains(&EmphasisFlag::Code);
    let strikethrough = stack.contains(&EmphasisFlag::Strike);

    let run = InlineRun {
        text: text.to_string(),
        bold,
        italic,
        code,
        strikethrough,
        link,
    };

    if let Some(last) = acc.last_mut()
        && same_style(last, &run)
    {
        last.text.push_str(text);
    } else {
        acc.push(run);
    }
}

fn same_style(a: &InlineRun, b: &InlineRun) -> bool {
    a.bold == b.bold
        && a.italic == b.italic
        && a.code == b.code
        && a.strikethrough == b.strikethrough
        && a.link == b.link
}

fn nest_into_quote(quote_depth: usize, blocks: &mut Vec<MdBlock>, block: MdBlock) {
    if quote_depth == 0 {
        blocks.push(block);
        return;
    }

    ensure_quote_depth(blocks, quote_depth);
    push_into_quote_at_depth(blocks, quote_depth, block);
}

/// Ensure a chain of `quote_depth` nested `BlockQuote` blocks exists at the tail.
fn ensure_quote_depth(blocks: &mut Vec<MdBlock>, depth: usize) {
    if depth == 0 {
        return;
    }

    let needs_new_quote = !matches!(blocks.last(), Some(MdBlock::BlockQuote { .. }));
    if needs_new_quote {
        blocks.push(MdBlock::BlockQuote {
            children: Vec::new(),
        });
    }

    if let Some(MdBlock::BlockQuote { children }) = blocks.last_mut() {
        ensure_quote_depth(children, depth - 1);
    }
}

/// Push `block` into the innermost quote at the given depth.
fn push_into_quote_at_depth(blocks: &mut Vec<MdBlock>, depth: usize, block: MdBlock) {
    if depth == 0 {
        blocks.push(block);
        return;
    }
    if let Some(MdBlock::BlockQuote { children }) = blocks.last_mut() {
        push_into_quote_at_depth(children, depth - 1, block);
    }
}

/// Convert a parsed block into a picus view.
fn block_to_view(
    block: MdBlock,
    base: &ResolvedStyle,
    palette: &MarkdownPalette,
) -> Box<picus_view::AnyWidgetView<(), ()>> {
    use picus_view::WidgetView as _;
    match block {
        MdBlock::Heading { level, runs } => {
            let (size, weight) = heading_style(level);
            sized_box(
                label(runs_to_string(&runs))
                    .text_size(size)
                    .weight(weight)
                    .color(palette.heading),
            )
            .padding(Padding::vertical(Length::px(4.0)))
            .boxed()
        }
        MdBlock::Paragraph { runs } => paragraph_view(&runs, base, palette),
        MdBlock::Code { language, code } => {
            code_block_view(language.as_deref(), &code, base, palette)
        }
        MdBlock::BlockQuote { children } => {
            let inner = children
                .into_iter()
                .map(|b| block_to_view(b, base, palette).into_any_flex())
                .collect::<Vec<_>>();
            sized_box(flex_col(inner).gap(Length::px(4.0)))
                .padding(Padding::horizontal(Length::px(12.0)))
                .border(palette.quote_border, Length::px(2.0))
                .corner_radius(Length::px(4.0))
                .boxed()
        }
        MdBlock::UnorderedList { items } => {
            let rows = items
                .into_iter()
                .map(|runs| {
                    let bullet = label("•").color(palette.muted);
                    let body = paragraph_view(&runs, base, palette);
                    flex_row(vec![
                        sized_box(bullet)
                            .width(Dim::Fixed(Length::px(16.0)))
                            .into_any_flex(),
                        body.into_any_flex(),
                    ])
                    .gap(Length::px(4.0))
                    .into_any_flex()
                })
                .collect::<Vec<_>>();
            flex_col(rows).gap(Length::px(2.0)).boxed()
        }
        MdBlock::OrderedList { start, items } => {
            let rows = items
                .into_iter()
                .enumerate()
                .map(|(i, runs)| {
                    let marker = label(format!("{}. ", start + i as u64)).color(palette.muted);
                    let body = paragraph_view(&runs, base, palette);
                    flex_row(vec![
                        sized_box(marker)
                            .width(Dim::Fixed(Length::px(24.0)))
                            .into_any_flex(),
                        body.into_any_flex(),
                    ])
                    .gap(Length::px(4.0))
                    .into_any_flex()
                })
                .collect::<Vec<_>>();
            flex_col(rows).gap(Length::px(2.0)).boxed()
        }
        MdBlock::Table {
            alignments,
            header,
            rows,
        } => table_view(alignments, header, rows, base, palette),
        MdBlock::ThematicBreak => sized_box(label(""))
            .width(Dim::Stretch)
            .height(Length::px(1.0))
            .background(palette.muted)
            .boxed(),
    }
}

fn heading_style(level: HeadingLevel) -> (f32, FontWeight) {
    match level {
        HeadingLevel::H1 => (28.0, FontWeight::BOLD),
        HeadingLevel::H2 => (24.0, FontWeight::BOLD),
        HeadingLevel::H3 => (20.0, FontWeight::SEMI_BOLD),
        HeadingLevel::H4 => (18.0, FontWeight::SEMI_BOLD),
        HeadingLevel::H5 => (16.0, FontWeight::SEMI_BOLD),
        HeadingLevel::H6 => (14.0, FontWeight::SEMI_BOLD),
    }
}

fn runs_to_string(runs: &[InlineRun]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect()
}

fn map_text_alignment(text_align: crate::styling::TextAlign) -> ParleyAlignment {
    match text_align {
        crate::styling::TextAlign::Start => ParleyAlignment::Start,
        crate::styling::TextAlign::Center => ParleyAlignment::Center,
        crate::styling::TextAlign::End => ParleyAlignment::End,
    }
}

fn table_alignment_to_text_align(alignment: Alignment) -> crate::styling::TextAlign {
    match alignment {
        Alignment::None | Alignment::Left => crate::styling::TextAlign::Start,
        Alignment::Center => crate::styling::TextAlign::Center,
        Alignment::Right => crate::styling::TextAlign::End,
    }
}

fn paragraph_view(
    runs: &[InlineRun],
    base: &ResolvedStyle,
    palette: &MarkdownPalette,
) -> Box<picus_view::AnyWidgetView<(), ()>> {
    use picus_view::WidgetView as _;
    if runs.is_empty() {
        return label("").boxed();
    }

    if runs.len() == 1 {
        return styled_label(&runs[0], base, palette);
    }

    let labels = runs
        .iter()
        .map(|run| styled_label(run, base, palette).into_any_flex())
        .collect::<Vec<_>>();
    flex_row(labels)
        .cross_axis_alignment(CrossAxisAlignment::FirstBaseline)
        .gap(Length::px(0.0))
        .boxed()
}

fn styled_label(
    run: &InlineRun,
    base: &ResolvedStyle,
    palette: &MarkdownPalette,
) -> Box<picus_view::AnyWidgetView<(), ()>> {
    use picus_view::WidgetView as _;
    let mut lbl = label(run.text.clone())
        .text_size(base.text.size)
        .text_alignment(map_text_alignment(base.text.text_align))
        .weight(FontWeight::new(base.text.weight))
        .line_height(LineHeight::FontSizeRelative(base.text.line_height));

    if let Some(font_stack) = font_stack_from_style(base) {
        lbl = lbl.font(font_stack);
    }

    if run.bold {
        lbl = lbl.weight(FontWeight::BOLD);
    } else if run.italic {
        lbl = lbl.weight(FontWeight::MEDIUM);
    }

    if run.italic {
        lbl = lbl.letter_spacing(0.2);
    }

    lbl = lbl.underline(run.link).strikethrough(run.strikethrough);

    let color = if run.link {
        palette.link
    } else if run.code {
        palette.code_text
    } else {
        palette.text
    };

    if run.code {
        lbl.text_size(13.0)
            .line_break_mode(picus_view::picus_widget::properties::LineBreaking::Overflow)
            .color(color)
            .background(palette.code_bg)
            .padding(Padding::all(Length::px(2.0)))
            .corner_radius(Length::px(3.0))
            .boxed()
    } else {
        lbl.color(color).boxed()
    }
}

fn code_block_view(
    language: Option<&str>,
    code: &str,
    _base: &ResolvedStyle,
    palette: &MarkdownPalette,
) -> Box<picus_view::AnyWidgetView<(), ()>> {
    use picus_view::WidgetView as _;
    let highlighted = highlight_code(language, code, palette.text);

    let lines = highlighted
        .into_iter()
        .map(|line| {
            let lbl = label(line.text)
                .text_size(13.0)
                .line_break_mode(picus_view::picus_widget::properties::LineBreaking::Overflow)
                .color(line.color);
            sized_box(lbl).width(Dim::Stretch).into_any_flex()
        })
        .collect::<Vec<_>>();

    sized_box(flex_col(lines).gap(Length::px(0.0)))
        .width(Dim::Stretch)
        .padding(Padding::all(Length::px(12.0)))
        .background(palette.pre_bg)
        .corner_radius(Length::px(6.0))
        .boxed()
}

fn table_view(
    alignments: Vec<Alignment>,
    header: Vec<Vec<InlineRun>>,
    rows: Vec<Vec<Vec<InlineRun>>>,
    base: &ResolvedStyle,
    palette: &MarkdownPalette,
) -> Box<picus_view::AnyWidgetView<(), ()>> {
    use picus_view::WidgetView as _;

    let mut rendered_rows = Vec::new();
    if !header.is_empty() {
        rendered_rows
            .push(table_row_view(header, &alignments, base, palette, true).into_any_flex());
    }
    rendered_rows.extend(
        rows.into_iter()
            .map(|row| table_row_view(row, &alignments, base, palette, false).into_any_flex()),
    );

    sized_box(flex_col(rendered_rows).gap(Length::px(0.0)))
        .width(Dim::Stretch)
        .border(palette.table_border, Length::px(1.0))
        .corner_radius(Length::px(4.0))
        .boxed()
}

fn table_row_view(
    cells: Vec<Vec<InlineRun>>,
    alignments: &[Alignment],
    base: &ResolvedStyle,
    palette: &MarkdownPalette,
    header: bool,
) -> Box<picus_view::AnyWidgetView<(), ()>> {
    use picus_view::WidgetView as _;

    let column_count = cells.len().max(alignments.len()).max(1);
    let cell_width = (720.0 / column_count as f64).clamp(96.0, 240.0);
    let cells = (0..column_count)
        .map(|index| {
            let runs = cells.get(index).cloned().unwrap_or_default();
            let mut cell_style = base.clone();
            cell_style.text.text_align = table_alignment_to_text_align(
                alignments.get(index).copied().unwrap_or(Alignment::None),
            );
            if header {
                cell_style.text.weight = 600.0;
            }

            let cell = sized_box(paragraph_view(&runs, &cell_style, palette))
                .width(Dim::Fixed(Length::px(cell_width)))
                .padding(Padding::all(Length::px(8.0)))
                .border(palette.table_border, Length::px(1.0));
            if header {
                cell.background(palette.table_header_bg).into_any_flex()
            } else {
                cell.into_any_flex()
            }
        })
        .collect::<Vec<_>>();

    flex_row(cells)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .boxed()
}

/// A single highlighted code line with its foreground color.
struct HighlightedLine {
    text: String,
    color: Color,
}

fn highlight_code(
    language: Option<&str>,
    code: &str,
    fallback_color: Color,
) -> Vec<HighlightedLine> {
    let state = highlight_state();

    let syntax = language
        .and_then(|lang| state.syntax_set.find_syntax_by_token(lang))
        .or_else(|| Some(state.syntax_set.find_syntax_plain_text()));

    let Some(syntax) = syntax else {
        return plain_code_lines(code, fallback_color);
    };

    let mut highlighter = syntect::easy::HighlightLines::new(syntax, &state.theme);

    let lines = if code.is_empty() {
        vec![""]
    } else {
        code.lines().collect::<Vec<_>>()
    };

    lines
        .into_iter()
        .map(
            |line| match highlighter.highlight_line(line, &state.syntax_set) {
                Ok(ranges) => {
                    let text = ranges.iter().map(|(_, s)| *s).collect::<String>();
                    let color = ranges
                        .iter()
                        .rev()
                        .find_map(|(style, _)| {
                            let fg = style.foreground;
                            if fg.a == 0 {
                                None
                            } else {
                                Some(Color::from_rgba8(fg.r, fg.g, fg.b, fg.a))
                            }
                        })
                        .unwrap_or(fallback_color);
                    HighlightedLine { text, color }
                }
                Err(_) => HighlightedLine {
                    text: line.to_string(),
                    color: fallback_color,
                },
            },
        )
        .collect()
}

fn plain_code_lines(code: &str, fallback_color: Color) -> Vec<HighlightedLine> {
    let lines = if code.is_empty() {
        vec![""]
    } else {
        code.lines().collect::<Vec<_>>()
    };

    lines
        .into_iter()
        .map(|line| HighlightedLine {
            text: line.to_string(),
            color: fallback_color,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TEXT: Color = Color::from_rgb8(0xE6, 0xE6, 0xE6);

    #[test]
    fn parses_heading_and_paragraph() {
        let blocks = parse_markdown_blocks("# Title\n\nSome text.");
        assert!(matches!(
            blocks.first(),
            Some(MdBlock::Heading {
                level: HeadingLevel::H1,
                ..
            })
        ));
        assert!(matches!(blocks.get(1), Some(MdBlock::Paragraph { .. })));
    }

    #[test]
    fn parses_fenced_code_block_with_language() {
        let md = "```rust\nfn main() {}\n```\n";
        let blocks = parse_markdown_blocks(md);
        let code = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::Code { language, code } => Some((language.clone(), code.clone())),
                _ => None,
            })
            .expect("should find a code block");
        assert_eq!(code.0.as_deref(), Some("rust"));
        assert!(code.1.contains("fn main"));
    }

    #[test]
    fn parses_unordered_list_items() {
        let md = "- one\n- two\n- three\n";
        let blocks = parse_markdown_blocks(md);
        let list = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::UnorderedList { items } => Some(items.len()),
                _ => None,
            })
            .expect("should find an unordered list");
        assert_eq!(list, 3);
    }

    #[test]
    fn parses_ordered_list_with_start() {
        let md = "3. first\n4. second\n";
        let blocks = parse_markdown_blocks(md);
        let (start, count) = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::OrderedList { start, items } => Some((*start, items.len())),
                _ => None,
            })
            .expect("should find an ordered list");
        assert_eq!(start, 3);
        assert_eq!(count, 2);
    }

    #[test]
    fn parses_block_quote_paragraph() {
        let md = "> quoted text\n";
        let blocks = parse_markdown_blocks(md);
        let quote = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::BlockQuote { children } => Some(children.len()),
                _ => None,
            })
            .expect("should find a block quote");
        assert_eq!(quote, 1);
    }

    #[test]
    fn parses_thematic_break() {
        let md = "a\n\n---\n\nb\n";
        let blocks = parse_markdown_blocks(md);
        assert!(blocks.iter().any(|b| matches!(b, MdBlock::ThematicBreak)));
    }

    #[test]
    fn parses_checked_task_list_marker() {
        let blocks = parse_markdown_blocks("- [x] Done\n- [ ] Later\n");
        let items = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::UnorderedList { items } => Some(items),
                _ => None,
            })
            .expect("should find a task list");

        assert_eq!(runs_to_string(&items[0]), "☑ Done");
        assert_eq!(runs_to_string(&items[1]), "☐ Later");
    }

    #[test]
    fn parses_gfm_table() {
        let md = "| Feature | Status |\n| :-- | --: |\n| Tables | Done |\n";
        let blocks = parse_markdown_blocks(md);
        let (alignments, header, rows) = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::Table {
                    alignments,
                    header,
                    rows,
                } => Some((alignments, header, rows)),
                _ => None,
            })
            .expect("should find a table");

        assert_eq!(alignments, &[Alignment::Left, Alignment::Right]);
        assert_eq!(header.len(), 2);
        assert_eq!(runs_to_string(&header[0]), "Feature");
        assert_eq!(rows.len(), 1);
        assert_eq!(runs_to_string(&rows[0][1]), "Done");
    }

    #[test]
    fn inline_runs_merge_same_style() {
        let mut acc = Vec::new();
        push_inline_run(&mut acc, &[], false, "hello ");
        push_inline_run(&mut acc, &[], false, "world");
        assert_eq!(acc.len(), 1);
        assert_eq!(acc[0].text, "hello world");
    }

    #[test]
    fn inline_runs_split_on_style_change() {
        let mut acc = Vec::new();
        push_inline_run(&mut acc, &[], false, "plain");
        push_inline_run(&mut acc, &[EmphasisFlag::Bold], false, "bold");
        push_inline_run(&mut acc, &[], false, "plain2");
        assert_eq!(acc.len(), 3);
        assert!(!acc[0].bold);
        assert!(acc[1].bold);
        assert!(!acc[2].bold);
    }

    #[test]
    fn highlight_code_returns_lines() {
        let lines = highlight_code(Some("rust"), "fn main() {}", TEST_TEXT);
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.text.contains("fn main")));
    }

    #[test]
    fn plain_code_lines_fallback_when_no_grammar() {
        let lines = highlight_code(Some("totally-not-a-language"), "x = 1", TEST_TEXT);
        assert!(!lines.is_empty());
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;
    use crate::UiStreamingMarkdown;

    #[test]
    fn streaming_append_accumulates_in_progress() {
        let mut s = UiStreamingMarkdown::new();
        s.append("Hello");
        s.append(", ");
        s.append("world!");
        assert_eq!(s.in_progress_source(), "Hello, world!");
        assert_eq!(s.completed_source(), "");
        assert!(!s.is_finished());
    }

    #[test]
    fn flush_completed_moves_in_progress_to_completed() {
        let mut s = UiStreamingMarkdown::new();
        s.append("# Title\n\n");
        s.flush_completed();
        assert_eq!(s.completed_source(), "# Title\n\n");
        assert_eq!(s.in_progress_source(), "");
        s.append("Some paragraph.");
        assert_eq!(s.in_progress_source(), "Some paragraph.");
        assert_eq!(s.completed_source(), "# Title\n\n");
    }

    #[test]
    fn finish_flushes_and_blocks_further_appends() {
        let mut s = UiStreamingMarkdown::new();
        s.append("done");
        s.finish();
        assert!(s.is_finished());
        assert_eq!(s.completed_source(), "done");
        assert_eq!(s.in_progress_source(), "");
        s.append("ignored");
        assert_eq!(s.completed_source(), "done");
    }

    #[test]
    fn full_source_combines_completed_and_in_progress() {
        let mut s = UiStreamingMarkdown::new();
        s.append("A");
        s.flush_completed();
        s.append("B");
        assert_eq!(s.full_source(), "AB");
    }

    #[test]
    fn cache_reuses_blocks_when_completed_source_unchanged() {
        let mut cache = StreamingMarkdownParseCache::default();
        let entity = Entity::from_bits(7);
        let src = "# H\n\npara\n";
        let first = cache.get_or_parse_completed(entity, src);
        let second = cache.get_or_parse_completed(entity, src);
        assert_eq!(first.len(), second.len());
    }

    #[test]
    fn cache_reparses_when_completed_source_changes() {
        let mut cache = StreamingMarkdownParseCache::default();
        let entity = Entity::from_bits(9);
        let first = cache.get_or_parse_completed(entity, "# A\n");
        let second = cache.get_or_parse_completed(entity, "# A\n\n# B\n");
        assert!(second.len() > first.len());
    }

    #[test]
    fn cache_evict_removes_entry() {
        let mut cache = StreamingMarkdownParseCache::default();
        let entity = Entity::from_bits(11);
        let _ = cache.get_or_parse_completed(entity, "x");
        assert!(cache.entries.contains_key(&entity));
        cache.evict(entity);
        assert!(!cache.entries.contains_key(&entity));
    }

    #[test]
    fn get_cached_returns_none_for_unknown_entity() {
        let cache = StreamingMarkdownParseCache::default();
        let entity = Entity::from_bits(13);
        assert!(cache.get_cached(entity, "x").is_none());
    }

    #[test]
    fn get_cached_returns_none_when_source_mismatch() {
        let mut cache = StreamingMarkdownParseCache::default();
        let entity = Entity::from_bits(15);
        let _ = cache.get_or_parse_completed(entity, "old");
        assert!(cache.get_cached(entity, "new").is_none());
    }
}

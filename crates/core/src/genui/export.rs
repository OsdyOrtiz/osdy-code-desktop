//! Exporting a surface as a file people keep: a diagram as Mermaid or SVG,
//! a table as CSV or Markdown, checklists and boards as Markdown, a chart
//! as an image (an SVG the app rasterizes to PNG) or CSV of its data, a
//! form's values as JSON, an html page as itself, sequence diagrams, data
//! models, state machines and timelines as Mermaid, and a review as a patch.

use std::fmt::Write as _;

use serde_json::{Map, Value, json};

use super::chart::{Axes, Chart, ChartType, Geometry, Shape, short_number};
use super::checklist::Checklist;
use super::diagram::{Diagram, EdgeStyle, NodeKind, Status};
use super::erd::Erd;
use super::form::Form;
use super::html::HtmlSpec;
use super::kanban::Board;
use super::layout::{self, Point, Rect};
use super::refs::CodeRef;
use super::review::{self, Review};
use super::sequence::{self, FragmentKind, MessageKind, ParticipantKind, Sequence};
use super::states::{Machine, StateKind};
use super::style::Tint;
use super::table::{Align, ColumnType, Table, cell_text, link_url};
use super::timeline::{self, Timeline};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Mermaid,
    Svg,
    Csv,
    Markdown,
    /// [`render`] gives the SVG to rasterize.
    Png,
    Json,
    Html,
    /// A unified diff (`git apply`).
    Patch,
}

impl Format {
    /// What menus call it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Mermaid => "Mermaid",
            Format::Svg => "SVG",
            Format::Csv => "CSV",
            Format::Markdown => "Markdown",
            Format::Png => "PNG",
            Format::Json => "JSON",
            Format::Html => "HTML",
            Format::Patch => "Patch",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Mermaid => "mmd",
            Format::Svg => "svg",
            Format::Csv => "csv",
            Format::Markdown => "md",
            Format::Png => "png",
            Format::Json => "json",
            Format::Html => "html",
            Format::Patch => "patch",
        }
    }

    /// The fence a Markdown document wraps it in (`None`: it is Markdown).
    pub fn fence(self) -> Option<&'static str> {
        match self {
            Format::Mermaid => Some("mermaid"),
            Format::Svg | Format::Html => Some("html"),
            Format::Csv => Some("csv"),
            Format::Json => Some("json"),
            Format::Markdown | Format::Png => None,
            Format::Patch => Some("diff"),
        }
    }
}

/// The formats a kind exports to, the one for docs first.
pub fn formats(kind: &str) -> &'static [Format] {
    match kind {
        "diagram" => &[Format::Mermaid, Format::Svg],
        "table" => &[Format::Markdown, Format::Csv],
        "checklist" | "kanban" => &[Format::Markdown],
        "chart" => &[Format::Png, Format::Csv],
        "form" => &[Format::Json],
        "html" => &[Format::Html],
        "sequence" | "erd" | "state" | "timeline" => &[Format::Mermaid],
        "review" => &[Format::Patch],
        _ => &[],
    }
}

/// The text format a kind goes into docs as.
pub fn docs_format(kind: &str) -> Option<Format> {
    formats(kind)
        .iter()
        .copied()
        .find(|f| *f != Format::Png && *f != Format::Svg)
}

/// Colors (CSS colors) and the font of drawn exports.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub background: String,
    pub surface: String,
    pub text: String,
    pub muted: String,
    pub border: String,
    pub accent: String,
    /// One per series or node kind tone, cycled.
    pub series: Vec<String>,
    /// One per [`Tint`], in [`Tint::ALL`] order: diagram elements' tones.
    pub tints: [String; 9],
    pub font: String,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            background: "#ffffff".into(),
            surface: "#f6f6f8".into(),
            text: "#1c1c22".into(),
            muted: "#6b6b76".into(),
            border: "#d4d4dc".into(),
            accent: "#d4577c".into(),
            series: [
                "#3b82f6", "#d4577c", "#16a34a", "#d97706", "#b08d57", "#c084fc", "#dc2626",
                "#6b7280",
            ]
            .map(String::from)
            .to_vec(),
            tints: [
                "#3b82f6", "#16a34a", "#d97706", "#dc2626", "#8b5cf6", "#0d9488", "#d4577c",
                "#b08d57", "#6b7280",
            ]
            .map(String::from),
            font: "sans-serif".into(),
        }
    }
}

/// `title` as a file name: a slug plus the format's extension.
pub fn file_name(title: &str, format: Format) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(60).collect();
    let slug = if slug.is_empty() { "ui".into() } else { slug };
    format!("{slug}.{}", format.extension())
}

/// An export error before it is formatted for the caller.
#[derive(Debug)]
pub enum Error {
    Unsupported { kind: String, format: Format },
    InvalidSpec(Vec<String>),
    NoDocsFormat(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { kind, format } => {
                write!(f, "a {kind} can't be exported as {}", format.name())
            }
            Self::InvalidSpec(errors) => f.write_str(&errors.join("; ")),
            Self::NoDocsFormat(kind) => write!(f, "a {kind} has no docs format"),
        }
    }
}

/// A surface's `spec` of `kind` as `format` (an SVG for [`Format::Png`]).
pub fn render(
    kind: &str,
    title: &str,
    spec: &Value,
    format: Format,
    palette: &Palette,
) -> Result<String, String> {
    render_detailed(kind, title, spec, format, palette).map_err(|error| error.to_string())
}

/// As [`render`], retaining diagnostics for display in the app.
pub fn render_detailed(
    kind: &str,
    title: &str,
    spec: &Value,
    format: Format,
    palette: &Palette,
) -> Result<String, Error> {
    if !formats(kind).contains(&format) {
        return Err(Error::Unsupported {
            kind: kind.to_string(),
            format,
        });
    }
    let errors = Error::InvalidSpec;
    Ok(match (kind, format) {
        ("diagram", Format::Mermaid) => mermaid(&Diagram::parse(spec).map_err(errors)?, title),
        ("diagram", Format::Svg) => diagram_svg(&Diagram::parse(spec).map_err(errors)?, palette),
        ("table", Format::Csv) => table_csv(&Table::parse(spec).map_err(errors)?),
        ("table", Format::Markdown) => table_markdown(&Table::parse(spec).map_err(errors)?),
        ("checklist", _) => checklist_markdown(&Checklist::parse(spec).map_err(errors)?),
        ("kanban", _) => kanban_markdown(&Board::parse(spec).map_err(errors)?),
        ("chart", Format::Csv) => chart_csv(&Chart::parse(spec).map_err(errors)?),
        ("chart", _) => chart_svg(&Chart::parse(spec).map_err(errors)?, title, palette),
        ("form", _) => form_json(&Form::parse(spec).map_err(errors)?),
        ("html", _) => HtmlSpec::parse(spec).map_err(errors)?.html,
        ("sequence", _) => sequence_mermaid(&Sequence::parse(spec).map_err(errors)?, title),
        ("erd", _) => erd_mermaid(&Erd::parse(spec).map_err(errors)?, title),
        ("state", _) => state_mermaid(&Machine::parse(spec).map_err(errors)?, title),
        ("timeline", _) => timeline_gantt(&Timeline::parse(spec).map_err(errors)?, title),
        ("review", _) => review_patch(&Review::parse(spec).map_err(errors)?),
        _ => unreachable!("checked against formats()"),
    })
}

/// Exports longer than this (bytes) aren't put in the docs request: the
/// agent has the spec already.
pub const MAX_DOCS_TEXT: usize = 60_000;

/// `/gentle-ui:event {"op":"docs", id, format, fence, text}`: the user
/// asks the agent to add the surface to the project's docs, in its
/// [`docs_format`] (the text left out when it's over [`MAX_DOCS_TEXT`]).
/// It carries no version, so the extension answers it as version 0, which
/// no edit being sent waits for.
pub fn docs_command(id: &str, kind: &str, title: &str, spec: &Value) -> Result<String, String> {
    docs_command_detailed(id, kind, title, spec).map_err(|error| error.to_string())
}

/// As [`docs_command`], retaining diagnostics separately from the wire command.
pub fn docs_command_detailed(
    id: &str,
    kind: &str,
    title: &str,
    spec: &Value,
) -> Result<String, Error> {
    let format = docs_format(kind).ok_or_else(|| Error::NoDocsFormat(kind.to_string()))?;
    let text = render_detailed(kind, title, spec, format, &Palette::default())?;
    let text = (text.len() <= MAX_DOCS_TEXT).then_some(text);
    let event = json!({
        "op": "docs",
        "id": id,
        "format": format.name(),
        "fence": format.fence(),
        "text": text,
    });
    Ok(format!("/{} {event}", super::EVENT_COMMAND))
}

// ── Mermaid ────────────────────────────────────────────────────────────────

/// A Mermaid front matter `title`, when there is one.
fn mermaid_title(out: &mut String, title: &str) {
    if !title.trim().is_empty() {
        let _ = writeln!(out, "---\ntitle: {}\n---", title.replace('\n', " "));
    }
}

/// A Mermaid `flowchart` id for each of `ids`: letters, digits and `_`,
/// prefixed so none is a keyword (`end`, `graph`), unique.
fn mermaid_ids<'a>(prefix: &str, ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let clean: String = id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let base = format!("{prefix}{clean}");
        let mut candidate = base.clone();
        let mut n = 2;
        while out.contains(&candidate) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        out.push(candidate);
    }
    out
}

/// Text inside a quoted Mermaid label.
fn mermaid_text(text: &str) -> String {
    text.replace('"', "#quot;")
        .replace('\n', "<br/>")
        .replace('\r', "")
}

/// The diagram as a Mermaid `flowchart`: groups as subgraphs, node kinds as
/// shapes, edge styles as arrows, statuses as classes, code refs as comments.
pub fn mermaid(d: &Diagram, title: &str) -> String {
    let nodes = mermaid_ids("n_", d.nodes.iter().map(|n| n.id.as_str()));
    let groups = mermaid_ids("g_", d.groups.iter().map(|g| g.id.as_str()));
    let node_id = |id: &str| d.node_index(id).map(|i| nodes[i].as_str());
    let mut out = String::new();
    mermaid_title(&mut out, title);
    let _ = writeln!(out, "flowchart {}", d.direction.name());
    let node_line = |out: &mut String, i: usize, indent: &str| {
        let node = &d.nodes[i];
        let mut label = mermaid_text(&node.label);
        if let Some(tech) = &node.tech {
            label.push_str(&format!("<br/><small>{}</small>", mermaid_text(tech)));
        }
        let (open, close) = match node.kind {
            NodeKind::Database | NodeKind::Storage => ("[(", ")]"),
            NodeKind::Queue => ("[[", "]]"),
            NodeKind::Cache => ("{{", "}}"),
            NodeKind::Client => ("([", "])"),
            NodeKind::External => (">", "]"),
            NodeKind::Function => ("(", ")"),
            NodeKind::Service | NodeKind::Note => ("[", "]"),
        };
        let _ = writeln!(out, "{indent}{}{open}\"{label}\"{close}", nodes[i]);
    };
    for (gi, group) in d.groups.iter().enumerate() {
        let members: Vec<usize> = (0..d.nodes.len())
            .filter(|&i| d.nodes[i].group.as_deref() == Some(group.id.as_str()))
            .collect();
        let _ = writeln!(
            out,
            "  subgraph {}[\"{}\"]",
            groups[gi],
            mermaid_text(&group.label)
        );
        for i in members {
            node_line(&mut out, i, "    ");
        }
        let _ = writeln!(out, "  end");
    }
    for (i, node) in d.nodes.iter().enumerate() {
        if node.group.is_none() {
            node_line(&mut out, i, "  ");
        }
    }
    for edge in &d.edges {
        let (Some(from), Some(to)) = (node_id(&edge.from), node_id(&edge.to)) else {
            continue;
        };
        let arrow = match edge.style {
            EdgeStyle::Sync => "-->",
            EdgeStyle::Async => "-.->",
            EdgeStyle::Data => "==>",
            EdgeStyle::Dependency => "--o",
        };
        match &edge.label {
            Some(label) => {
                let _ = writeln!(out, "  {from} {arrow}|\"{}\"| {to}", mermaid_text(label));
            }
            None => {
                let _ = writeln!(out, "  {from} {arrow} {to}");
            }
        }
    }
    let classes = [
        (Status::Planned, "stroke-dasharray: 5 5"),
        (Status::Deprecated, "opacity: 0.55"),
        (Status::Risk, "stroke: #dc2626, stroke-width: 2px"),
    ];
    for (status, style) in classes {
        let ids: Vec<&str> = (0..d.nodes.len())
            .filter(|&i| d.nodes[i].status == Some(status))
            .map(|i| nodes[i].as_str())
            .collect();
        if !ids.is_empty() {
            let _ = writeln!(out, "  classDef {} {style}", status.name());
            let _ = writeln!(out, "  class {} {}", ids.join(","), status.name());
        }
    }
    for (i, node) in d.nodes.iter().enumerate() {
        if let Some(code_ref) = &node.code_ref {
            let _ = writeln!(out, "  %% {}: {}", nodes[i], code_ref.text());
        }
    }
    out
}

// ── SVG ───────────────────────────────────────────────────────────────────

fn xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// `text` cut to `max` characters for a fixed-size box.
fn fit(text: &str, max: usize) -> String {
    super::diagram::truncate(text, max)
}

/// `text` broken at spaces into at most `lines` lines of `width` characters.
/// When more is left, the last line ends with `…`, dropping its last word if
/// there is no room for it; words longer than a line are cut.
fn wrap(text: &str, width: usize, lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let word = fit(word, width);
        let full = out.len() == lines;
        match out.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(&word);
            }
            Some(line) if full => {
                if line.ends_with('…') {
                    break;
                }
                if line.chars().count() + 1 > width {
                    match line.rfind(' ') {
                        Some(space) => line.truncate(space),
                        None => *line = line.chars().take(width.saturating_sub(1)).collect(),
                    }
                }
                line.push('…');
                break;
            }
            _ => out.push(word),
        }
    }
    out
}

fn svg_open(out: &mut String, w: f32, h: f32, palette: &Palette) {
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}" font-family="{}">"#,
        xml(&palette.font)
    );
    let _ = writeln!(
        out,
        r#"<rect width="100%" height="100%" fill="{}"/>"#,
        palette.background
    );
}

fn tone(palette: &Palette, i: usize) -> &str {
    palette
        .series
        .get(i % palette.series.len().max(1))
        .map_or(palette.accent.as_str(), String::as_str)
}

/// A diagram element's named color in the palette.
fn tint(palette: &Palette, tint: Tint) -> &str {
    // `tints` follows `Tint::ALL`, the declaration order.
    &palette.tints[tint as usize]
}

/// A node kind's default tone, as the app draws it.
fn kind_tint(kind: NodeKind) -> Tint {
    match kind {
        NodeKind::Service => Tint::Blue,
        NodeKind::Function => Tint::Purple,
        NodeKind::Database => Tint::Green,
        NodeKind::Storage => Tint::Teal,
        NodeKind::Queue => Tint::Yellow,
        NodeKind::Cache => Tint::Red,
        NodeKind::Client => Tint::Pink,
        NodeKind::External => Tint::Gray,
        NodeKind::Note => Tint::Gold,
    }
}

/// An edge's color: its `color`, else its style's.
fn edge_color<'a>(edge: &super::diagram::Edge, palette: &'a Palette) -> &'a str {
    match (edge.color, edge.style) {
        (Some(color), _) => tint(palette, color),
        (None, EdgeStyle::Async) => tint(palette, Tint::Yellow),
        (None, EdgeStyle::Data) => tint(palette, Tint::Blue),
        (None, EdgeStyle::Sync | EdgeStyle::Dependency) => palette.muted.as_str(),
    }
}

/// The diagram as laid out in the app, as a standalone SVG: tinted cards
/// with their icon badge, groups as tinted containers with a caps tag,
/// edges in their style's (or `color`'s) tone ending in open chevrons, and
/// labels in pills.
pub fn diagram_svg(d: &Diagram, palette: &Palette) -> String {
    const MARGIN: f32 = 24.;
    // Card corner radius, badge side and group corner radius, as in the app.
    const CARD_RADIUS: f32 = 12.;
    const BADGE: f32 = 26.;
    const GROUP_RADIUS: f32 = 16.;
    // Where a group's tag sits inside its box, and the tag's height.
    const TAG_INSET: f32 = 10.;
    const TAG_H: f32 = 18.;
    let l = layout::layout(d);
    let b = l.bounds;
    let (ox, oy) = (MARGIN - b.x, MARGIN - b.y);
    let at = |p: Point| (p.x + ox, p.y + oy);
    // SVG 1.1 markers can't take their line's color: one per color used.
    let mut colors: Vec<&str> = Vec::new();
    for edge in &d.edges {
        let color = edge_color(edge, palette);
        if !colors.contains(&color) {
            colors.push(color);
        }
    }
    let mut out = String::new();
    svg_open(
        &mut out,
        b.w.max(1.) + MARGIN * 2.,
        b.h.max(1.) + MARGIN * 2.,
        palette,
    );
    out.push_str(
        r##"<defs><filter id="shadow" x="-10%" y="-10%" width="120%" height="140%"><feDropShadow dx="0" dy="3" stdDeviation="5" flood-color="#000000" flood-opacity="0.18"/></filter>"##,
    );
    for (i, color) in colors.iter().enumerate() {
        let _ = write!(
            out,
            r#"<marker id="arrow-{i}" viewBox="0 0 12 12" refX="10" refY="6" markerWidth="12" markerHeight="12" markerUnits="userSpaceOnUse" orient="auto-start-reverse"><path d="M2,1.5 L10,6 L2,10.5" fill="none" stroke="{color}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></marker>"#
        );
    }
    out.push_str("</defs>\n");
    for (group, rect) in d.groups.iter().zip(&l.groups) {
        let Some(r) = rect else { continue };
        let (x, y) = at(Point { x: r.x, y: r.y });
        let color = group
            .color
            .map_or(palette.muted.as_str(), |c| tint(palette, c));
        let _ = writeln!(
            out,
            r#"<rect x="{x:.1}" y="{y:.1}" width="{:.1}" height="{:.1}" rx="{GROUP_RADIUS}" fill="{color}" fill-opacity="0.05" stroke="{color}" stroke-opacity="0.42" stroke-width="1.5"/>"#,
            r.w, r.h,
        );
        // The tag: the label in caps on its tone, top-left inside the box.
        let label = fit(&group.label, 40).to_uppercase();
        let w = (label.chars().count() as f32 * 6.4 + 12.).min(r.w - 2. * TAG_INSET);
        let (tx, ty) = (x + TAG_INSET, y + TAG_INSET);
        let _ = writeln!(
            out,
            r#"<rect x="{tx:.1}" y="{ty:.1}" width="{w:.1}" height="{TAG_H}" rx="5" fill="{color}" fill-opacity="0.16" stroke="{color}" stroke-opacity="0.4"/><text x="{:.1}" y="{:.1}" font-size="9.5" font-weight="600" fill="{color}">{}</text>"#,
            tx + 6.,
            ty + 12.5,
            xml(&label)
        );
    }
    for ((edge, path), spot) in d.edges.iter().zip(&l.edges).zip(&l.labels) {
        let mut data = String::new();
        if let Some(first) = path.segments.first() {
            let (x, y) = at(first.from);
            let _ = write!(data, "M{x:.1},{y:.1}");
        }
        for s in &path.segments {
            let (c1, c2, to) = (at(s.c1), at(s.c2), at(s.to));
            let _ = write!(
                data,
                " C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                c1.0, c1.1, c2.0, c2.1, to.0, to.1
            );
        }
        let (dash, width) = match edge.style {
            EdgeStyle::Sync => ("", 1.5),
            EdgeStyle::Async => (r#" stroke-dasharray="6 4""#, 1.5),
            EdgeStyle::Data => (r#" stroke-dasharray="10 5""#, 2.25),
            EdgeStyle::Dependency => (r#" stroke-dasharray="2 4""#, 1.5),
        };
        let color = edge_color(edge, palette);
        let marker = colors.iter().position(|c| *c == color).unwrap_or(0);
        let _ = writeln!(
            out,
            r#"<path d="{data}" fill="none" stroke="{color}" stroke-width="{width}"{dash} marker-end="url(#arrow-{marker})"/>"#
        );
        if let Some(label) = &edge.label {
            let (x, y) = at(spot.unwrap_or_else(|| path.label_at()));
            let lines = wrap(label, 24, 2);
            let style = (edge.style != EdgeStyle::Sync).then(|| edge.style.name());
            let rows = lines.len() + usize::from(style.is_some());
            let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * 6. + 12.;
            let h = rows as f32 * 13. + 4.;
            let top = y - h / 2.;
            // A pill in the canvas color keeps the line out of the text.
            let _ = write!(
                out,
                r#"<rect x="{:.1}" y="{top:.1}" width="{w:.1}" height="{h:.1}" rx="6" fill="{}" stroke="{}"/>"#,
                x - w / 2.,
                palette.background,
                palette.border,
            );
            for (i, line) in lines.iter().enumerate() {
                let _ = write!(
                    out,
                    r#"<text x="{x:.1}" y="{:.1}" font-size="10.5" text-anchor="middle" fill="{}" fill-opacity="0.82">{}</text>"#,
                    top + 12. + i as f32 * 13.,
                    palette.text,
                    xml(line)
                );
            }
            if let Some(style) = style {
                let _ = write!(
                    out,
                    r#"<text x="{x:.1}" y="{:.1}" font-size="9" font-family="monospace" text-anchor="middle" fill="{}">{style}</text>"#,
                    top + 12. + lines.len() as f32 * 13.,
                    palette.muted,
                );
            }
            out.push('\n');
        }
    }
    for (node, r) in d.nodes.iter().zip(&l.nodes) {
        let (x, y) = at(Point { x: r.x, y: r.y });
        let note = node.kind == NodeKind::Note;
        // Notes and external systems sit flat, without the shadow.
        let flat = note || node.kind == NodeKind::External;
        let color = tint(palette, node.color.unwrap_or(kind_tint(node.kind)));
        let dash = if node.status == Some(Status::Planned) {
            r#" stroke-dasharray="5 4""#
        } else {
            ""
        };
        let (stroke, stroke_opacity) = if node.status == Some(Status::Risk) {
            (tint(palette, Tint::Red), 0.7)
        } else {
            (color, 0.55)
        };
        let opacity = if node.status == Some(Status::Deprecated) {
            r#" opacity="0.55""#
        } else {
            ""
        };
        let shadow = if flat {
            ""
        } else {
            r#" filter="url(#shadow)""#
        };
        // An opaque base under the tint, so edges don't show through.
        let _ = write!(
            out,
            r#"<g{opacity}><rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="{CARD_RADIUS}" fill="{}"{shadow}/><rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="{CARD_RADIUS}" fill="{color}" fill-opacity="0.10" stroke="{stroke}" stroke-opacity="{stroke_opacity}"{dash}/>"#,
            palette.surface,
            w = r.w,
            h = r.h,
        );
        // The badge: a solid square in the tone (the icon stays in the app).
        let _ = write!(
            out,
            r#"<rect x="{:.1}" y="{:.1}" width="{BADGE}" height="{BADGE}" rx="7.8" fill="{color}"/>"#,
            x + 12.,
            y + 8.,
        );
        let _ = write!(
            out,
            r#"<text x="{:.1}" y="{:.1}" font-size="13" font-weight="600" fill="{}">{}</text>"#,
            x + 12. + BADGE + 8.,
            y + 8. + BADGE / 2. + 4.5,
            palette.text,
            xml(&fit(&node.label, 20)),
        );
        let italic = if note { r#" font-style="italic""# } else { "" };
        let description = node.description.as_deref().unwrap_or_default();
        for (i, line) in wrap(description, 32, 2).iter().enumerate() {
            let _ = write!(
                out,
                r#"<text x="{:.1}" y="{:.1}" font-size="11"{italic} fill="{}">{}</text>"#,
                x + 12.,
                y + 48. + i as f32 * 13.,
                palette.muted,
                xml(line),
            );
        }
        let meta = match node.tech.as_deref().filter(|t| !t.is_empty()) {
            Some(tech) => format!("{} · {tech}", node.kind.name().to_uppercase()),
            None => node.kind.name().to_uppercase(),
        };
        let _ = write!(
            out,
            r#"<text x="{:.1}" y="{:.1}" font-size="9.5" font-family="monospace" fill="{}">{}</text>"#,
            x + 12.,
            y + r.h - 10.,
            palette.muted,
            xml(&fit(&meta, 34)),
        );
        out.push_str("</g>\n");
    }
    out.push_str("</svg>\n");
    out
}

// ── Tables ────────────────────────────────────────────────────────────────

/// One CSV field (RFC 4180): quoted when it holds a comma, a quote, a line
/// break or edge spaces; quotes doubled.
pub fn csv_field(text: &str) -> String {
    let needs =
        text.contains([',', '"', '\n', '\r']) || text.starts_with(' ') || text.ends_with(' ');
    if needs {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn csv_line(out: &mut String, fields: impl IntoIterator<Item = String>) {
    let fields: Vec<String> = fields.into_iter().map(|f| csv_field(&f)).collect();
    out.push_str(&fields.join(","));
    out.push('\n');
}

fn has_refs(table: &Table) -> bool {
    table.rows.iter().any(|r| r.code_ref.is_some())
}

/// The table as CSV: a header of column labels, one line per row (links as
/// their URL), and a trailing `ref` column when rows point at code.
pub fn table_csv(table: &Table) -> String {
    let refs = has_refs(table);
    let mut out = String::new();
    csv_line(
        &mut out,
        table
            .columns
            .iter()
            .map(|c| c.label.clone())
            .chain(refs.then(|| "ref".to_string())),
    );
    for row in &table.rows {
        let cells = table.columns.iter().map(|c| {
            let value = row.cells.get(&c.key).unwrap_or(&Value::Null);
            match c.kind {
                ColumnType::Link => link_url(value).map_or_else(|| cell_text(value), String::from),
                _ => cell_text(value),
            }
        });
        let code = refs.then(|| {
            row.code_ref
                .as_ref()
                .map_or_else(String::new, CodeRef::text)
        });
        csv_line(&mut out, cells.chain(code));
    }
    out
}

/// Text inside a Markdown table cell.
fn md_cell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace('\n', "<br>")
}

/// Inline Markdown text (outside tables): line breaks become spaces.
fn md_inline(text: &str) -> String {
    text.replace("\r\n", " ").replace('\n', " ")
}

fn md_ref(code_ref: &CodeRef) -> String {
    format!("`{}`", code_ref.text().replace('`', "'"))
}

/// The table as a GitHub-flavored Markdown table, aligned like its columns.
pub fn table_markdown(table: &Table) -> String {
    let refs = has_refs(table);
    let mut out = String::new();
    let mut header: Vec<String> = table.columns.iter().map(|c| md_cell(&c.label)).collect();
    let mut rule: Vec<&str> = table
        .columns
        .iter()
        .map(|c| match c.align {
            Align::Left => "---",
            Align::Center => ":---:",
            Align::Right => "---:",
        })
        .collect();
    if refs {
        header.push("Code".into());
        rule.push("---");
    }
    let _ = writeln!(out, "| {} |", header.join(" | "));
    let _ = writeln!(out, "| {} |", rule.join(" | "));
    for row in &table.rows {
        let mut cells: Vec<String> = table
            .columns
            .iter()
            .map(|c| {
                let value = row.cells.get(&c.key).unwrap_or(&Value::Null);
                let text = md_cell(&cell_text(value));
                match (c.kind, link_url(value)) {
                    (ColumnType::Link, Some(url)) => {
                        format!("[{text}]({})", url.replace(' ', "%20"))
                    }
                    _ => text,
                }
            })
            .collect();
        if refs {
            cells.push(row.code_ref.as_ref().map_or_else(String::new, md_ref));
        }
        let _ = writeln!(out, "| {} |", cells.join(" | "));
    }
    out
}

// ── Checklists and boards ─────────────────────────────────────────────────

/// The checklist as a Markdown task list (sub-items indented).
pub fn checklist_markdown(list: &Checklist) -> String {
    let mut out = String::new();
    for (parent, item) in list.flat() {
        let indent = if parent.is_some() { "  " } else { "" };
        let _ = write!(
            out,
            "{indent}- [{}] {}",
            if item.done { "x" } else { " " },
            md_inline(&item.label)
        );
        if let Some(code_ref) = &item.code_ref {
            let _ = write!(out, " ({})", md_ref(code_ref));
        }
        if let Some(note) = &item.note {
            let _ = write!(out, " — {}", md_inline(note));
        }
        out.push('\n');
    }
    out
}

/// The board as Markdown: a heading per column and its cards as a list.
pub fn kanban_markdown(board: &Board) -> String {
    let mut out = String::new();
    for (i, column) in board.columns.iter().enumerate() {
        let cards: Vec<_> = board
            .cards
            .iter()
            .filter(|c| c.column == column.id)
            .collect();
        if i > 0 {
            out.push('\n');
        }
        let count = match column.limit {
            Some(limit) => format!("{}/{limit}", cards.len()),
            None => cards.len().to_string(),
        };
        let _ = writeln!(out, "### {} ({count})\n", md_inline(&column.label));
        if cards.is_empty() {
            out.push_str("_No cards_\n");
        }
        for card in cards {
            let _ = write!(out, "- **{}**", md_inline(&card.title));
            if let Some(description) = &card.description {
                let _ = write!(out, " — {}", md_inline(description));
            }
            for tag in &card.tags {
                let _ = write!(out, " `{}`", tag.replace('`', "'"));
            }
            if let Some(assignee) = &card.assignee {
                let _ = write!(out, " @{}", md_inline(assignee));
            }
            if let Some(code_ref) = &card.code_ref {
                let _ = write!(out, " ({})", md_ref(code_ref));
            }
            out.push('\n');
        }
    }
    out
}

// ── Charts ────────────────────────────────────────────────────────────────

fn number_text(n: f64) -> String {
    super::edits::format_number(n)
}

/// The chart's data as CSV: `label,<series…>` per label, or `series,x,y`
/// per point for scatter.
pub fn chart_csv(chart: &Chart) -> String {
    let mut out = String::new();
    if chart.kind == ChartType::Scatter {
        csv_line(&mut out, ["series", "x", "y"].map(String::from));
        for s in &chart.series {
            for (x, y) in &s.points {
                csv_line(&mut out, [s.name.clone(), number_text(*x), number_text(*y)]);
            }
        }
        return out;
    }
    let series: Vec<_> = if chart.kind.is_round() {
        chart.series.iter().take(1).collect()
    } else {
        chart.series.iter().collect()
    };
    csv_line(
        &mut out,
        std::iter::once(chart.x_title.clone().unwrap_or_else(|| "label".into()))
            .chain(series.iter().map(|s| s.name.clone())),
    );
    for (i, label) in chart.labels.iter().enumerate() {
        csv_line(
            &mut out,
            std::iter::once(label.clone()).chain(series.iter().map(|s| {
                s.values
                    .get(i)
                    .map_or_else(String::new, |v| number_text(*v))
            })),
        );
    }
    out
}

/// The chart drawn like the app draws it, as a standalone SVG (720 × 440)
/// with its title, axes and a legend.
pub fn chart_svg(chart: &Chart, title: &str, palette: &Palette) -> String {
    const W: f32 = 720.;
    const H: f32 = 440.;
    let mut out = String::new();
    svg_open(&mut out, W, H, palette);
    let _ = writeln!(
        out,
        r#"<text x="24" y="32" font-size="16" font-weight="600" fill="{}">{}</text>"#,
        palette.text,
        xml(&fit(title, 80))
    );
    let legend = chart.kind.is_round() || chart.series.len() > 1;
    let bottom = if legend { 74. } else { 48. };
    let axes = Axes::of(chart);
    let plot = if chart.kind.is_round() {
        Rect {
            x: 24.,
            y: 52.,
            w: W - 48.,
            h: H - 52. - bottom,
        }
    } else {
        Rect {
            x: 72.,
            y: 52.,
            w: W - 96.,
            h: H - 52. - bottom,
        }
    };
    let g = Geometry::new(chart, axes.as_ref(), plot, 1.);
    if let Some(axes) = &axes {
        for tick in &axes.y.ticks {
            let y = plot.y + plot.h * (1. - axes.y.fraction(*tick));
            let _ = writeln!(
                out,
                r#"<line x1="{:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="{}"/><text x="{:.1}" y="{:.1}" font-size="11" text-anchor="end" fill="{}">{}</text>"#,
                plot.x,
                plot.right(),
                palette.border,
                plot.x - 8.,
                y + 4.,
                palette.muted,
                xml(&short_number(*tick))
            );
        }
        match &axes.x {
            Some(x_axis) => {
                for tick in &x_axis.ticks {
                    let x = plot.x + plot.w * x_axis.fraction(*tick);
                    let _ = writeln!(
                        out,
                        r#"<text x="{x:.1}" y="{:.1}" font-size="11" text-anchor="middle" fill="{}">{}</text>"#,
                        plot.bottom() + 18.,
                        palette.muted,
                        xml(&short_number(*tick))
                    );
                }
            }
            None => {
                for (label, x) in chart.labels.iter().zip(&g.columns) {
                    let _ = writeln!(
                        out,
                        r#"<text x="{x:.1}" y="{:.1}" font-size="11" text-anchor="middle" fill="{}">{}</text>"#,
                        plot.bottom() + 18.,
                        palette.muted,
                        xml(&fit(label, 16))
                    );
                }
            }
        }
        if let Some(y_title) = &chart.y_title {
            let _ = writeln!(
                out,
                r#"<text x="16" y="{:.1}" font-size="11" fill="{}" transform="rotate(-90 16 {:.1})" text-anchor="middle">{}</text>"#,
                plot.y + plot.h / 2.,
                palette.muted,
                plot.y + plot.h / 2.,
                xml(y_title)
            );
        }
        if let Some(x_title) = &chart.x_title {
            let _ = writeln!(
                out,
                r#"<text x="{:.1}" y="{:.1}" font-size="11" text-anchor="middle" fill="{}">{}</text>"#,
                plot.x + plot.w / 2.,
                plot.bottom() + 36.,
                palette.muted,
                xml(x_title)
            );
        }
    }
    let points = |line: &[Point]| {
        line.iter()
            .map(|p| format!("{:.1},{:.1}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ")
    };
    if chart.kind == ChartType::Area {
        for (si, (line, floor)) in g.lines.iter().zip(&g.floors).enumerate() {
            let mut shape = points(line);
            let back: Vec<Point> = floor.iter().rev().copied().collect();
            shape.push(' ');
            shape.push_str(&points(&back));
            let _ = writeln!(
                out,
                r#"<polygon points="{shape}" fill="{}" fill-opacity="0.25"/>"#,
                tone(palette, si)
            );
        }
    }
    if matches!(chart.kind, ChartType::Line | ChartType::Area) {
        for (si, line) in g.lines.iter().enumerate() {
            let _ = writeln!(
                out,
                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2"/>"#,
                points(line),
                tone(palette, si)
            );
        }
    }
    for mark in &g.marks {
        let color = if chart.kind.is_round() {
            tone(palette, mark.index)
        } else {
            tone(palette, mark.series)
        };
        match &mark.shape {
            Shape::Bar(r) => {
                let _ = writeln!(
                    out,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="2" fill="{color}"/>"#,
                    r.x, r.y, r.w, r.h
                );
            }
            Shape::Dot(p) => {
                let _ = writeln!(
                    out,
                    r#"<circle cx="{:.1}" cy="{:.1}" r="3.5" fill="{color}"/>"#,
                    p.x, p.y
                );
            }
            Shape::Slice {
                center,
                radius,
                inner,
                start,
                end,
            } => {
                let _ = writeln!(
                    out,
                    r#"<path d="{}" fill="{color}" stroke="{}" stroke-width="1.5"/>"#,
                    slice_path(*center, *radius, *inner, *start, *end),
                    palette.background
                );
            }
        }
    }
    if legend {
        let names: Vec<String> = if chart.kind.is_round() {
            chart.labels.clone()
        } else {
            chart.series.iter().map(|s| s.name.clone()).collect()
        };
        let mut x = 24.;
        let y = H - 24.;
        for (i, name) in names.iter().enumerate() {
            let text = fit(name, 24);
            let w = text.chars().count() as f32 * 6.6 + 30.;
            if x + w > W - 24. {
                break;
            }
            let _ = writeln!(
                out,
                r#"<rect x="{x:.1}" y="{:.1}" width="10" height="10" rx="2" fill="{}"/><text x="{:.1}" y="{y:.1}" font-size="11" fill="{}">{}</text>"#,
                y - 9.,
                tone(palette, i),
                x + 15.,
                palette.muted,
                xml(&text)
            );
            x += w;
        }
    }
    out.push_str("</svg>\n");
    out
}

/// A pie or donut slice, angles clockwise from 12 o'clock.
fn slice_path(c: Point, r: f32, inner: f32, start: f32, end: f32) -> String {
    let at = |radius: f32, angle: f32| (c.x + radius * angle.sin(), c.y - radius * angle.cos());
    // A full circle as two halves (an arc can't start where it ends).
    let sweep = (end - start).min(std::f32::consts::TAU - 1e-3);
    let end = start + sweep;
    let large = i32::from(sweep > std::f32::consts::PI);
    let (sx, sy) = at(r, start);
    let (ex, ey) = at(r, end);
    if inner <= 0. {
        return format!(
            "M{:.1},{:.1} L{sx:.1},{sy:.1} A{r:.1},{r:.1} 0 {large} 1 {ex:.1},{ey:.1} Z",
            c.x, c.y
        );
    }
    let (ix, iy) = at(inner, end);
    let (jx, jy) = at(inner, start);
    format!(
        "M{sx:.1},{sy:.1} A{r:.1},{r:.1} 0 {large} 1 {ex:.1},{ey:.1} L{ix:.1},{iy:.1} A{inner:.1},{inner:.1} 0 {large} 0 {jx:.1},{jy:.1} Z"
    )
}

// ── Forms ─────────────────────────────────────────────────────────────────

/// The form's values (answers, else defaults) by field id, in field order.
pub fn form_json(form: &Form) -> String {
    let values: Map<String, Value> = form
        .fields
        .iter()
        .map(|f| (f.id.clone(), form.value(&f.id)))
        .collect();
    let mut text = serde_json::to_string_pretty(&Value::Object(values)).unwrap_or_default();
    text.push('\n');
    text
}

/// Free text after a `:` in a sequence or state diagram: one line, and
/// `;`/`#` (statement and entity syntax) as entities.
fn mermaid_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '#' => out.push_str("#35;"),
            ';' => out.push_str("#59;"),
            '\r' => {}
            '\n' => out.push_str("<br/>"),
            c => out.push(c),
        }
    }
    out
}

/// The sequence as a Mermaid `sequenceDiagram`: actors and participants,
/// messages by kind (`->>` sync, `-)` async, `-->>` return), notes,
/// `alt`/`opt`/`loop`/`par` blocks with their `else`/`and` branches and
/// the activation bars.
pub fn sequence_mermaid(seq: &Sequence, title: &str) -> String {
    let ids = mermaid_ids("p_", seq.participants.iter().map(|p| p.id.as_str()));
    let who = |id: &str| seq.lane(id).map_or("?", |i| ids[i].as_str());
    let mut out = String::new();
    mermaid_title(&mut out, title);
    out.push_str("sequenceDiagram\n");
    for (i, p) in seq.participants.iter().enumerate() {
        let keyword = if p.kind == ParticipantKind::Actor {
            "actor"
        } else {
            "participant"
        };
        let _ = writeln!(out, "  {keyword} {} as {}", ids[i], mermaid_line(&p.label));
    }
    // Blocks by message place; nested ones open outermost first.
    let mut blocks: Vec<(usize, usize, &sequence::Fragment)> = seq
        .fragments
        .iter()
        .filter_map(|f| {
            let (a, b) = (seq.index(&f.first)?, seq.index(&f.last)?);
            Some((a.min(b), a.max(b), f))
        })
        .collect();
    blocks.sort_by_key(|(a, b, _)| (*a, std::cmp::Reverse(*b)));
    let bars = seq.bars();
    for (mi, m) in seq.messages.iter().enumerate() {
        for (_, _, f) in blocks.iter().filter(|(a, ..)| *a == mi) {
            let label = f.label.as_deref().map(mermaid_line).unwrap_or_default();
            let _ = writeln!(out, "  {} {label}", f.kind.name());
        }
        for (a, _, f) in &blocks {
            let divider = match f.kind {
                FragmentKind::Alt => "else",
                FragmentKind::Par => "and",
                FragmentKind::Opt | FragmentKind::Loop => continue,
            };
            for branch in f.branches.iter().filter(|b| b.at == m.id && *a < mi) {
                let label = branch
                    .label
                    .as_deref()
                    .map(mermaid_line)
                    .unwrap_or_default();
                let _ = writeln!(out, "  {divider} {label}");
            }
        }
        let arrow = match m.kind {
            MessageKind::Sync | MessageKind::SelfCall => "->>",
            MessageKind::Async => "-)",
            MessageKind::Return => "-->>",
        };
        let (from, to) = (who(&m.from), who(&m.to));
        let _ = writeln!(out, "  {from}{arrow}{to}: {}", mermaid_line(&m.label));
        if let Some(note) = &m.note {
            let place = if from == to {
                format!("right of {from}")
            } else {
                format!("over {from},{to}")
            };
            let _ = writeln!(out, "  Note {place}: {}", mermaid_line(note));
        }
        for (lane, ..) in bars.iter().filter(|(_, first, _)| *first == mi) {
            let _ = writeln!(out, "  activate {}", ids[*lane]);
        }
        for (lane, ..) in bars.iter().filter(|(_, _, last)| *last == mi) {
            let _ = writeln!(out, "  deactivate {}", ids[*lane]);
        }
        for _ in blocks.iter().filter(|(_, b, _)| *b == mi) {
            out.push_str("  end\n");
        }
    }
    out
}

/// A Mermaid `erDiagram` word (entity names and attribute types): letters,
/// digits, `_`, `-` and the brackets of `varchar(255)` / `text[]`.
fn mermaid_word(text: &str, fallback: &str) -> String {
    let word: String = text
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_-()[]".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    match word.chars().next() {
        None => fallback.to_string(),
        Some(c) if c.is_ascii_alphabetic() || c == '_' => word,
        Some(_) => format!("_{word}"),
    }
}

/// The data model as a Mermaid `erDiagram`: each table with its columns
/// (`PK`/`FK`/`UK`, notes and nullability as comments), each relation with
/// crow's-foot ends, code refs as `%%` comments.
pub fn erd_mermaid(erd: &Erd, title: &str) -> String {
    let names = mermaid_ids("", erd.tables.iter().map(|t| t.name.as_str()));
    let entity = |id: &str| erd.table_index(id).map_or("?", |i| names[i].as_str());
    let mut out = String::new();
    mermaid_title(&mut out, title);
    out.push_str("erDiagram\n");
    for (i, t) in erd.tables.iter().enumerate() {
        let _ = writeln!(out, "  {} {{", mermaid_word(&names[i], "table"));
        for c in &t.columns {
            let keys: Vec<&str> = [
                (c.pk, "PK"),
                (c.fk.is_some(), "FK"),
                (c.unique && !c.pk, "UK"),
            ]
            .into_iter()
            .filter_map(|(on, key)| on.then_some(key))
            .collect();
            let mut line = format!(
                "    {} {}",
                mermaid_word(c.ty.as_deref().unwrap_or(""), "any"),
                mermaid_word(&c.name, "column")
            );
            if !keys.is_empty() {
                let _ = write!(line, " {}", keys.join(", "));
            }
            let comment: Vec<&str> = c
                .note
                .as_deref()
                .into_iter()
                .chain(c.nullable.then_some("nullable"))
                .collect();
            if !comment.is_empty() {
                let _ = write!(line, " \"{}\"", comment.join("; ").replace('"', "'"));
            }
            let _ = writeln!(out, "{line}");
        }
        out.push_str("  }\n");
    }
    for r in &erd.relations {
        let (from_many, to_many) = r.cardinality.many();
        let left = if from_many { "}o" } else { "||" };
        let right = if to_many { "o{" } else { "||" };
        let label = r
            .label
            .clone()
            .unwrap_or_else(|| format!("{} → {}", r.from.column, r.to.column));
        let _ = writeln!(
            out,
            "  {} {left}--{right} {} : \"{}\"",
            mermaid_word(entity(&r.from.table), "table"),
            mermaid_word(entity(&r.to.table), "table"),
            label.replace('"', "'").replace('\n', " ")
        );
    }
    for (i, t) in erd.tables.iter().enumerate() {
        if let Some(code_ref) = &t.code_ref {
            let _ = writeln!(out, "  %% {}: {}", names[i], code_ref.text());
        }
    }
    out
}

/// The state machine as a Mermaid `stateDiagram-v2`: initial and final
/// states as `[*]`, choices as `<<choice>>`, composite groups as nested
/// `state … { }` blocks, transitions labelled `event [guard] / action`,
/// the current state as the `current` class.
pub fn state_mermaid(m: &Machine, title: &str) -> String {
    let ids = mermaid_ids("s_", m.states.iter().map(|s| s.id.as_str()));
    let groups = mermaid_ids("g_", m.groups.iter().map(|g| g.id.as_str()));
    let end = |id: &str| match m.state_index(id) {
        Some(i) if matches!(m.states[i].kind, StateKind::Initial | StateKind::Final) => "[*]",
        Some(i) => ids[i].as_str(),
        None => "?",
    };
    let group_of = |id: &str| m.state(id).and_then(|s| s.group.as_deref());
    let mut out = String::new();
    mermaid_title(&mut out, title);
    out.push_str("stateDiagram-v2\n");
    let _ = writeln!(out, "  direction {}", m.direction.name());
    // One scope's states and the transitions that stay inside it.
    let scope = |out: &mut String, group: Option<&str>, indent: &str| {
        for (i, s) in m.states.iter().enumerate() {
            if s.group.as_deref() != group {
                continue;
            }
            match s.kind {
                StateKind::Initial | StateKind::Final => {}
                StateKind::Choice => {
                    let _ = writeln!(out, "{indent}state {} <<choice>>", ids[i]);
                }
                StateKind::Normal => {
                    let label = s.label.replace('"', "'").replace('\n', " ");
                    let _ = writeln!(out, "{indent}state \"{label}\" as {}", ids[i]);
                }
            }
            if let Some(description) = &s.description
                && !s.kind.pseudo()
            {
                let _ = writeln!(out, "{indent}{} : {}", ids[i], mermaid_line(description));
            }
        }
        for t in &m.transitions {
            let inside = group_of(&t.from) == group && group_of(&t.to) == group;
            if !inside {
                continue;
            }
            let label = t.label();
            let _ = write!(out, "{indent}{} --> {}", end(&t.from), end(&t.to));
            if !label.is_empty() {
                let _ = write!(out, " : {}", mermaid_line(&label));
            }
            out.push('\n');
        }
    };
    for (gi, g) in m.groups.iter().enumerate() {
        let label = g.label.replace('"', "'").replace('\n', " ");
        let _ = writeln!(out, "  state \"{label}\" as {} {{", groups[gi]);
        scope(&mut out, Some(g.id.as_str()), "    ");
        out.push_str("  }\n");
    }
    scope(&mut out, None, "  ");
    // Transitions between scopes, by their real states ([*] is per scope).
    for t in &m.transitions {
        if group_of(&t.from) == group_of(&t.to) {
            continue;
        }
        let side = |id: &str| m.state_index(id).map_or("?", |i| ids[i].as_str());
        let label = t.label();
        let _ = write!(out, "  {} --> {}", side(&t.from), side(&t.to));
        if !label.is_empty() {
            let _ = write!(out, " : {}", mermaid_line(&label));
        }
        out.push('\n');
    }
    if let Some(i) = m.current.as_deref().and_then(|id| m.state_index(id)) {
        let _ = writeln!(
            out,
            "  classDef current stroke-width:3px\n  class {} current",
            ids[i]
        );
    }
    out
}

/// The timeline as a Mermaid `gantt`: tasks with their dates and length in
/// days, groups as sections, done/active from the progress, the critical
/// path as `crit`, milestones; dependencies as `%%` comments (the dates
/// are explicit, so Mermaid's `after` would move nothing).
pub fn timeline_gantt(timeline: &Timeline, title: &str) -> String {
    let ids = mermaid_ids("t_", timeline.tasks.iter().map(|t| t.id.as_str()));
    let critical = timeline.critical_path();
    let mut out = String::new();
    mermaid_title(&mut out, title);
    out.push_str("gantt\n  dateFormat YYYY-MM-DD\n");
    for group in timeline.groups() {
        if let Some(group) = group {
            let _ = writeln!(
                out,
                "  section {}",
                group.replace([':', '#', ';', '\n'], " ")
            );
        }
        for (i, t) in timeline.tasks.iter().enumerate() {
            if t.group.as_deref() != group {
                continue;
            }
            let mut tags: Vec<&str> = Vec::new();
            match t.progress {
                Some(100) => tags.push("done"),
                Some(p) if p > 0 => tags.push("active"),
                _ => {}
            }
            if critical.contains(&t.id) {
                tags.push("crit");
            }
            if t.milestone {
                tags.push("milestone");
            }
            let length = if t.milestone { 0 } else { t.end - t.start + 1 };
            tags.push(&ids[i]);
            let _ = writeln!(
                out,
                "  {} :{}, {}, {length}d",
                t.label.replace([':', '#', ';', '\n'], " "),
                tags.join(", "),
                timeline::date_of(t.start)
            );
            if !t.depends_on.is_empty() {
                let _ = writeln!(out, "  %% {} after {}", ids[i], t.depends_on.join(", "));
            }
        }
    }
    out
}

// ── Reviews ───────────────────────────────────────────────────────────────

/// The review as one unified patch (`git apply`-able): every file's hunks
/// as proposed, whatever their decisions.
pub fn review_patch(review: &Review) -> String {
    review.files.iter().map(review::File::patch).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn diagram() -> Diagram {
        Diagram::parse(&json!({
            "direction": "TB",
            "groups": [{"id": "backend", "label": "Back \"end\""}],
            "nodes": [
                {"id": "web", "label": "Web app", "kind": "client"},
                {"id": "api", "label": "API", "group": "backend", "tech": "Rust", "status": "planned", "ref": "src/api.rs:12"},
                {"id": "end", "label": "Orders DB", "kind": "database", "group": "backend"},
                {"id": "q-1", "label": "Jobs", "kind": "queue"},
                {"id": "q_1", "label": "Jobs 2", "kind": "queue"}
            ],
            "edges": [
                {"from": "web", "to": "api", "label": "HTTPS | json"},
                {"from": "api", "to": "end", "style": "data"},
                {"from": "api", "to": "q-1", "style": "async"},
                {"from": "q-1", "to": "q_1", "style": "dependency"}
            ]
        }))
        .unwrap()
    }

    /// Mermaid read back structurally: the header, balanced subgraphs,
    /// every node declared once and every edge between declared nodes.
    #[test]
    fn mermaid_is_a_well_formed_flowchart() {
        let text = mermaid(&diagram(), "Checkout");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            &lines[..4],
            ["---", "title: Checkout", "---", "flowchart TB"]
        );
        let mut depth = 0;
        let mut declared: Vec<String> = Vec::new();
        let mut edges = 0;
        for line in &lines[4..] {
            let line = line.trim();
            if line.starts_with("subgraph ") {
                depth += 1;
                assert!(line.contains("[\"Back #quot;end#quot;\"]"), "{line}");
                continue;
            }
            if line == "end" {
                depth -= 1;
                assert!(depth >= 0);
                continue;
            }
            if line.starts_with("classDef") || line.starts_with("class ") || line.starts_with("%%")
            {
                continue;
            }
            let id: String = line
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            assert!(id.starts_with("n_"), "{line}");
            let rest = &line[id.len()..];
            if let Some(arrow) = ["-->", "-.->", "==>", "--o"]
                .iter()
                .find(|a| rest.trim_start().starts_with(**a))
            {
                let target = rest
                    .trim_start()
                    .trim_start_matches(arrow)
                    .rsplit(' ')
                    .next()
                    .unwrap();
                assert!(declared.contains(&id), "{id} used before it is declared");
                assert!(
                    declared.iter().any(|d| d == target),
                    "unknown target {target}"
                );
                edges += 1;
            } else {
                assert!(!declared.contains(&id), "{id} declared twice");
                declared.push(id);
            }
        }
        assert_eq!(depth, 0);
        assert_eq!(declared.len(), 5);
        assert_eq!(edges, 4);
        // `end` is a keyword: ids are prefixed; `q-1` and `q_1` stay apart.
        assert!(text.contains("n_end[(\"Orders DB\")]"));
        assert!(text.contains("n_q_1[[\"Jobs\"]]") && text.contains("n_q_1_2[[\"Jobs 2\"]]"));
        assert!(text.contains("n_web -->|\"HTTPS | json\"| n_api"));
        assert!(text.contains("n_api ==> n_end") && text.contains("n_api -.-> n_q_1"));
        assert!(text.contains("n_api[\"API<br/><small>Rust</small>\"]"));
        assert!(text.contains("class n_api planned"));
        assert!(text.contains("%% n_api: src/api.rs:12"));
    }

    #[test]
    fn diagram_svg_draws_every_node_and_edge() {
        let svg = diagram_svg(&diagram(), &Palette::default());
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("marker-end=").count(), 4);
        // One open chevron per edge color: muted (sync, dependency), the
        // data edge's blue and the async edge's yellow.
        assert_eq!(svg.matches("<marker id=").count(), 3);
        assert!(svg.contains(">Orders DB</text>"));
        // A database card wears green: tinted fill and its badge.
        assert!(svg.contains(r##"fill="#16a34a" fill-opacity="0.10""##));
        assert!(svg.contains(r##"rx="7.8" fill="#16a34a""##));
        // The group's tag is in caps.
        assert!(svg.contains("BACK &quot;END&quot;"));
        assert!(svg.contains("HTTPS | json"));
        assert!(svg.contains(">SERVICE · Rust</text>"));
    }

    #[test]
    fn wrap_breaks_at_spaces_and_ellipsizes_the_rest() {
        assert_eq!(wrap("", 10, 2), Vec::<String>::new());
        assert_eq!(wrap("Sends events", 24, 2), ["Sends events"]);
        assert_eq!(
            wrap("Builds the window menus and global actions", 16, 2),
            ["Builds the", "window menus…"]
        );
        assert_eq!(wrap("a b", 1, 2), ["a", "b"]);
    }

    fn table() -> Table {
        Table::parse(&json!({
            "columns": [
                {"key": "name", "label": "Name"},
                {"key": "n", "label": "Count", "type": "number"},
                {"key": "url", "label": "Link", "type": "link"}
            ],
            "rows": [
                {"id": "a", "cells": {"name": "plain", "n": 3, "url": {"text": "docs", "url": "https://x.dev/a b"}}, "ref": "src/a.rs:4"},
                {"id": "b", "cells": {"name": "with, comma \"quoted\"\nand a | pipe", "n": 1.5}}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn csv_escapes_fields() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("two\nlines"), "\"two\nlines\"");
        assert_eq!(csv_field(" padded"), "\" padded\"");
        assert_eq!(
            table_csv(&table()),
            "Name,Count,Link,ref\n\
             plain,3,https://x.dev/a b,src/a.rs:4\n\
             \"with, comma \"\"quoted\"\"\nand a | pipe\",1.5,,\n"
        );
    }

    #[test]
    fn markdown_tables_align_and_escape() {
        assert_eq!(
            table_markdown(&table()),
            "| Name | Count | Link | Code |\n\
             | --- | ---: | --- | --- |\n\
             | plain | 3 | [docs](https://x.dev/a%20b) | `src/a.rs:4` |\n\
             | with, comma \"quoted\"<br>and a \\| pipe | 1.5 |  |  |\n"
        );
    }

    #[test]
    fn checklists_and_boards_as_markdown() {
        let list = Checklist::parse(&json!({"items": [
            {"id": "plan", "label": "Plan", "done": true, "note": "with\nnotes"},
            {"id": "build", "label": "Build", "items": [{"id": "api", "label": "API", "ref": "src/api.rs"}]}
        ]}))
        .unwrap();
        assert_eq!(
            checklist_markdown(&list),
            "- [x] Plan — with notes\n- [ ] Build\n  - [ ] API (`src/api.rs`)\n"
        );
        let board = Board::parse(&json!({
            "columns": [{"id": "todo", "label": "To do", "limit": 2}, "Done"],
            "cards": [{"id": "c", "title": "OAuth", "description": "login", "tags": ["auth"], "assignee": "Ada", "ref": "src/auth.rs:9"}]
        }))
        .unwrap();
        assert_eq!(
            kanban_markdown(&board),
            "### To do (1/2)\n\n- **OAuth** — login `auth` @Ada (`src/auth.rs:9`)\n\n### Done (0)\n\n_No cards_\n"
        );
    }

    #[test]
    fn chart_data_and_picture() {
        let chart = Chart::parse(&json!({
            "type": "bar", "labels": ["Mon", "Tue"],
            "series": [{"name": "api", "values": [120, 98.5]}, {"name": "web, edge", "values": [3, 4]}]
        }))
        .unwrap();
        assert_eq!(
            chart_csv(&chart),
            "label,api,\"web, edge\"\nMon,120,3\nTue,98.5,4\n"
        );
        let svg = chart_svg(&chart, "p95 <latency>", &Palette::default());
        assert_eq!(
            svg.matches("<rect x=").count() - 2,
            4,
            "4 bars and 2 legend keys"
        );
        assert!(svg.contains("p95 &lt;latency&gt;"));
        let scatter = Chart::parse(
            &json!({"type": "scatter", "series": [{"name": "s", "points": [[1, 2], [3.5, 4]]}]}),
        )
        .unwrap();
        assert_eq!(chart_csv(&scatter), "series,x,y\ns,1,2\ns,3.5,4\n");
        let pie = Chart::parse(&json!({"type": "donut", "labels": ["a", "b"], "series": [{"name": "s", "values": [1, 3]}]})).unwrap();
        assert_eq!(
            chart_svg(&pie, "", &Palette::default())
                .matches("<path d=\"M")
                .count(),
            2
        );
    }

    #[test]
    fn form_values_as_json() {
        let form = Form::parse(&json!({
            "fields": [
                {"id": "env", "type": "radio", "label": "Env", "options": ["staging", "prod"]},
                {"id": "notes", "type": "textarea", "label": "Notes"}
            ],
            "values": {"env": "prod"}
        }))
        .unwrap();
        let value: Value = serde_json::from_str(&form_json(&form)).unwrap();
        assert_eq!(value["env"], "prod");
        assert!(value.get("notes").is_some());
    }

    #[test]
    fn render_checks_the_format_and_names_files() {
        let spec = json!({"html": "<p>hi</p>"});
        assert_eq!(
            render("html", "x", &spec, Format::Html, &Palette::default()).unwrap(),
            "<p>hi</p>"
        );
        assert!(render("html", "x", &spec, Format::Csv, &Palette::default()).is_err());
        assert!(render("table", "x", &json!({}), Format::Csv, &Palette::default()).is_err());
        assert_eq!(
            file_name("Checkout — v2!", Format::Mermaid),
            "checkout-v2.mmd"
        );
        assert_eq!(file_name("", Format::Png), "ui.png");
        assert_eq!(docs_format("chart"), Some(Format::Csv));
        assert_eq!(docs_format("diagram"), Some(Format::Mermaid));
    }

    #[test]
    fn docs_command_carries_the_docs_format() {
        let spec = json!({"nodes": [{"id": "api"}]});
        let command = docs_command("arch", "diagram", "Arch", &spec).unwrap();
        let event: Value =
            serde_json::from_str(command.strip_prefix("/gentle-ui:event ").unwrap()).unwrap();
        assert_eq!(event["op"], "docs");
        assert_eq!(event["id"], "arch");
        assert!(event.get("version").is_none());
        assert_eq!(
            (event["format"].as_str(), event["fence"].as_str()),
            (Some("Mermaid"), Some("mermaid"))
        );
        assert!(event["text"].as_str().unwrap().contains("flowchart LR"));
        let big = json!({"html": "x".repeat(MAX_DOCS_TEXT + 1)});
        let event: Value = serde_json::from_str(
            docs_command("p", "html", "Page", &big)
                .unwrap()
                .strip_prefix("/gentle-ui:event ")
                .unwrap(),
        )
        .unwrap();
        assert!(event["text"].is_null());
    }

    /// Each `open` line is closed by an `end` line, in order.
    fn balanced(text: &str, opens: &[&str]) {
        let mut depth = 0i32;
        for line in text.lines().map(str::trim) {
            let word = line.split(' ').next().unwrap_or("");
            if opens.contains(&word) {
                depth += 1;
            } else if line == "end" {
                depth -= 1;
                assert!(depth >= 0, "end without a block in\n{text}");
            }
        }
        assert_eq!(depth, 0, "unclosed block in\n{text}");
    }

    #[test]
    fn sequence_as_mermaid() {
        let seq = Sequence::parse(&json!({
            "participants": [{"id": "web", "label": "Web", "kind": "actor"}, {"id": "api", "label": "API"}, {"id": "db", "label": "Users; DB", "kind": "db"}],
            "messages": [
                {"id": "m1", "from": "web", "to": "api", "label": "POST /login"},
                {"id": "m2", "from": "api", "to": "db", "label": "find user", "note": "by #email"},
                {"id": "m3", "from": "db", "to": "api", "label": "row", "kind": "return"},
                {"id": "m4", "from": "api", "to": "web", "label": "401", "kind": "return"},
                {"id": "m5", "from": "api", "to": "web", "label": "200", "kind": "return"},
                {"id": "m6", "from": "api", "to": "api", "label": "audit", "kind": "async"}
            ],
            "fragments": [
                {"kind": "alt", "label": "wrong password", "from": "m4", "to": "m5", "else": [{"at": "m5", "label": "ok"}]},
                {"kind": "loop", "label": "retries", "from": "m1", "to": "m5"}
            ]
        }))
        .unwrap();
        let text = sequence_mermaid(&seq, "Login");
        assert!(text.starts_with("---\ntitle: Login\n---\nsequenceDiagram\n"));
        assert!(text.contains("  actor p_web as Web\n"));
        assert!(text.contains("  participant p_db as Users#59; DB\n"));
        assert!(text.contains("  p_web->>p_api: POST /login\n"));
        assert!(text.contains("  Note over p_api,p_db: by #35;email\n"));
        assert!(text.contains("  p_db-->>p_api: row\n"));
        // A message to itself is a self call.
        assert!(text.contains("  p_api->>p_api: audit\n"));
        // The loop opens before the alt it holds; `else` splits the alt.
        let at = |needle: &str| text.find(needle).unwrap();
        assert!(at("  loop retries") < at("  alt wrong password"));
        assert!(at("  p_api-->>p_web: 401") < at("  else ok") && at("  else ok") < at(": 200"));
        balanced(&text, &["loop", "alt", "opt", "par"]);
        // The derived activation bars open and close in pairs.
        assert_eq!(
            text.matches("  activate ").count(),
            text.matches("  deactivate ").count()
        );
        assert!(text.contains("  activate p_api\n"));
    }

    #[test]
    fn erd_as_mermaid() {
        let erd = Erd::parse(&json!({"tables": [
            {"id": "users", "ref": "db/001_users.sql:3", "columns": [
                {"name": "id", "type": "uuid", "pk": true},
                {"name": "email", "type": "varchar(255)", "unique": true, "note": "login \"name\""}
            ]},
            {"id": "orders", "columns": [
                {"name": "id", "type": "uuid", "pk": true},
                {"name": "user_id", "type": "uuid", "fk": "users.id", "nullable": true},
                {"name": "total", "type": "numeric 10,2"}
            ]}
        ]}))
        .unwrap();
        let text = erd_mermaid(&erd, "");
        assert!(text.starts_with("erDiagram\n"));
        assert!(text.contains(
            "  users {\n    uuid id PK\n    varchar(255) email UK \"login 'name'\"\n  }\n"
        ));
        assert!(text.contains("    uuid user_id FK \"nullable\"\n"));
        assert!(text.contains("    numeric_10_2 total\n"));
        // The foreign key is many orders to one user.
        assert!(text.contains("  orders }o--|| users : \"user_id → id\"\n"));
        assert!(text.contains("  %% users: db/001_users.sql:3\n"));
        assert_eq!(text.matches('{').count(), text.matches('}').count() - 1);
    }

    #[test]
    fn state_machine_as_mermaid() {
        let machine = Machine::parse(&json!({
            "direction": "LR",
            "groups": [{"id": "review", "label": "In review"}],
            "states": [
                {"id": "start", "kind": "initial"},
                {"id": "draft", "label": "Draft", "description": "not shared"},
                {"id": "checks", "label": "Checks", "group": "review"},
                {"id": "approved", "label": "Approved", "group": "review"},
                {"id": "pick", "kind": "choice"},
                {"id": "done", "kind": "final"}
            ],
            "transitions": [
                {"from": "start", "to": "draft"},
                {"from": "draft", "to": "checks", "event": "submit", "guard": "ci green", "action": "notify"},
                {"from": "checks", "to": "approved", "event": "approve"},
                {"from": "approved", "to": "pick"},
                {"from": "pick", "to": "done", "event": "merge"}
            ],
            "current": "checks"
        }))
        .unwrap();
        let text = state_mermaid(&machine, "PR");
        assert!(text.contains("stateDiagram-v2\n  direction LR\n"));
        assert!(
            text.contains(
                "  state \"In review\" as g_review {\n    state \"Checks\" as s_checks\n"
            )
        );
        assert!(text.contains("    s_checks --> s_approved : approve\n  }\n"));
        assert!(text.contains("  state s_pick <<choice>>\n"));
        assert!(text.contains("  s_draft : not shared\n"));
        assert!(text.contains("  [*] --> s_draft\n"));
        assert!(text.contains("  s_pick --> [*] : merge\n"));
        assert!(text.contains("  s_draft --> s_checks : submit [ci green] / notify\n"));
        assert!(text.contains("  s_approved --> s_pick\n"));
        assert!(text.contains("  class s_checks current\n"));
    }

    #[test]
    fn timeline_as_gantt() {
        let timeline = Timeline::parse(&json!({"tasks": [
            {"id": "design", "label": "Design: v1", "start": "2026-03-02", "duration": 5, "progress": 1},
            {"id": "build", "label": "Build", "group": "Eng", "start": "2026-03-09", "end": "2026-03-20", "dependsOn": ["design"], "progress": 30},
            {"id": "launch", "label": "Launch", "start": "2026-03-23", "milestone": true, "dependsOn": "build"}
        ]}))
        .unwrap();
        let text = timeline_gantt(&timeline, "Launch plan");
        assert!(text.contains("gantt\n  dateFormat YYYY-MM-DD\n"));
        assert!(text.contains("  Design  v1 :done, crit, t_design, 2026-03-02, 5d\n"));
        assert!(text.contains("  Launch :crit, milestone, t_launch, 2026-03-23, 0d\n"));
        assert!(text.contains("  section Eng\n  Build :active, crit, t_build, 2026-03-09, 12d\n"));
        assert!(text.contains("  %% t_build after design\n"));
        // Ungrouped tasks come before the first section.
        assert!(text.find("Launch :").unwrap() < text.find("section Eng").unwrap());
    }

    #[test]
    fn review_as_a_patch() {
        let spec = json!({"files": [
            {"path": "src/auth.rs", "hunks": [{"id": "sig", "oldStart": 1, "newStart": 1, "lines": [" fn main() {", "-    login();", "+    sign_in();", " }"], "decision": "rejected"}]},
            {"path": "NEW.md", "status": "added", "hunks": [{"oldStart": 0, "newStart": 1, "lines": ["+hello"]}]}
        ]});
        let text = render(
            "review",
            "Rename",
            &spec,
            Format::Patch,
            &Palette::default(),
        )
        .unwrap();
        assert_eq!(
            text,
            "diff --git a/src/auth.rs b/src/auth.rs\n--- a/src/auth.rs\n+++ b/src/auth.rs\n@@ -1,3 +1,3 @@\n fn main() {\n-    login();\n+    sign_in();\n }\n\
             diff --git a/NEW.md b/NEW.md\nnew file mode 100644\n--- /dev/null\n+++ b/NEW.md\n@@ -0,0 +1,1 @@\n+hello\n"
        );
        assert_eq!(docs_format("review"), Some(Format::Patch));
        assert_eq!(Format::Patch.fence(), Some("diff"));
        for kind in ["sequence", "erd", "state", "timeline"] {
            assert_eq!(docs_format(kind), Some(Format::Mermaid));
        }
    }
}

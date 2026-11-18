//! Export d'un arbre de vue en page HTML statique (aperçu, `mcf view`).
//!
//! Même règle que la cible web (D61) : le texte est échappé, les styles sont posés un à un à
//! partir de valeurs déjà typées (aucune chaîne du `.uimcf` n'est copiée dans le CSS), et les
//! images ne sont pas chargées (un cadre montre le texte alternatif et la source).

use std::fmt::Write;

use mcf_view::{Kind, Length, Node, Prop, Style, StyleValue};

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

pub fn page(title: &str, root: &Node) -> String {
    let mut body = String::new();
    node(root, None, &mut body);
    format!(
        "<!doctype html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\n\
<title>{}</title>\n<style>{BASE_CSS}</style>\n</head>\n<body>\n{body}</body>\n</html>\n",
        escape(title)
    )
}

pub const BASE_CSS: &str = "\
html,body{margin:0;min-height:100%;background:#11111b;color:#cdd6f4;\
font:16px/1.4 system-ui,-apple-system,'Segoe UI',sans-serif}\
*{box-sizing:border-box}\
.mcf-box,.mcf-window{display:flex;flex-direction:column}\
.mcf-window{min-height:100vh}\
button.mcf-button{font:inherit;color:inherit;background:#313244;border:1px solid #45475a;\
border-radius:6px;padding:6px 14px;cursor:pointer}\
input.mcf-input{font:inherit;color:inherit;background:#1e1e2e;border:1px solid #45475a;\
border-radius:6px;padding:6px 10px}\
.mcf-image{display:inline-flex;flex-direction:column;align-items:center;justify-content:center;\
min-width:64px;min-height:64px;background:repeating-linear-gradient(45deg,#1e1e2e,#1e1e2e 8px,#262638 8px,#262638 16px);\
border:1px dashed #585b70;color:#a6adc8;font-size:12px;padding:6px;text-align:center}";

fn length(l: Length) -> String {
    match l {
        Length::Px(n) => format!("{n}px"),
        Length::Percent(p) => format!("{p}%"),
    }
}

fn word(style: Style, w: &str) -> &str {
    match (style, w) {
        (Style::Align, "start") | (Style::Justify, "start") => "flex-start",
        (Style::Align, "end") | (Style::Justify, "end") => "flex-end",
        (Style::Justify, "between") => "space-between",
        (Style::Justify, "around") => "space-around",
        (_, w) => w,
    }
}

/// Déclarations CSS d'un style ; `parent_stack` : le parent superpose ses enfants.
pub fn css(n: &Node, parent_stack: bool) -> String {
    let mut css = String::new();
    let mut p = |name: &str, v: String| {
        let _ = write!(css, "{name}:{v};");
    };
    if parent_stack {
        p("grid-area", "1/1".into());
    }
    for (style, value) in &n.style {
        match (style, value) {
            (Style::Width, StyleValue::Length(l)) => p("width", length(*l)),
            (Style::Height, StyleValue::Length(l)) => p("height", length(*l)),
            (Style::MinWidth, StyleValue::Length(l)) => p("min-width", length(*l)),
            (Style::MaxWidth, StyleValue::Length(l)) => p("max-width", length(*l)),
            (Style::MinHeight, StyleValue::Length(l)) => p("min-height", length(*l)),
            (Style::MaxHeight, StyleValue::Length(l)) => p("max-height", length(*l)),
            (Style::Padding, StyleValue::Length(l)) => p("padding", length(*l)),
            (Style::PaddingTop, StyleValue::Length(l)) => p("padding-top", length(*l)),
            (Style::PaddingRight, StyleValue::Length(l)) => p("padding-right", length(*l)),
            (Style::PaddingBottom, StyleValue::Length(l)) => p("padding-bottom", length(*l)),
            (Style::PaddingLeft, StyleValue::Length(l)) => p("padding-left", length(*l)),
            (Style::Margin, StyleValue::Length(l)) => p("margin", length(*l)),
            (Style::MarginTop, StyleValue::Length(l)) => p("margin-top", length(*l)),
            (Style::MarginRight, StyleValue::Length(l)) => p("margin-right", length(*l)),
            (Style::MarginBottom, StyleValue::Length(l)) => p("margin-bottom", length(*l)),
            (Style::MarginLeft, StyleValue::Length(l)) => p("margin-left", length(*l)),
            (Style::Gap, StyleValue::Length(l)) => p("gap", length(*l)),
            (Style::BorderWidth, StyleValue::Length(l)) => {
                p("border-width", length(*l));
                p("border-style", "solid".into());
            }
            (Style::Radius, StyleValue::Length(l)) => p("border-radius", length(*l)),
            (Style::FontSize, StyleValue::Length(l)) => p("font-size", length(*l)),
            (Style::X, StyleValue::Length(l)) => {
                p("position", "absolute".into());
                p("left", length(*l));
            }
            (Style::Y, StyleValue::Length(l)) => {
                p("position", "absolute".into());
                p("top", length(*l));
            }
            (Style::Background, StyleValue::Color(c)) => p("background", c.to_string()),
            (Style::Color, StyleValue::Color(c)) => p("color", c.to_string()),
            (Style::BorderColor, StyleValue::Color(c)) => p("border-color", c.to_string()),
            (Style::FontWeight, StyleValue::Weight(w)) => p("font-weight", w.to_string()),
            (Style::Opacity, StyleValue::Opacity(o)) => p("opacity", o.to_string()),
            (Style::Grow, StyleValue::Grow(g)) => p("flex-grow", g.to_string()),
            (Style::Visible, StyleValue::Bool(false)) => p("display", "none".into()),
            (Style::Layout, StyleValue::Word(w)) => match w.as_str() {
                "row" => p("flex-direction", "row".into()),
                "stack" => p("display", "grid".into()),
                _ => p("flex-direction", "column".into()),
            },
            (Style::Align, StyleValue::Word(w)) => p("align-items", word(*style, w).into()),
            (Style::Justify, StyleValue::Word(w)) => p("justify-content", word(*style, w).into()),
            (Style::TextAlign, StyleValue::Word(w)) => p("text-align", w.clone()),
            (Style::Overflow, StyleValue::Word(w)) => p(
                "overflow",
                if w == "scroll" { "auto" } else { w.as_str() }.into(),
            ),
            _ => {}
        }
    }
    css
}

fn node(n: &Node, parent_stack: Option<bool>, out: &mut String) {
    let style = css(n, parent_stack == Some(true));
    let style_attr = if style.is_empty() {
        String::new()
    } else {
        format!(" style=\"{style}\"")
    };
    let text = |p: Prop| escape(n.props.get(&p).map(String::as_str).unwrap_or(""));
    match n.kind {
        Kind::Window | Kind::Box => {
            let (tag, class) = if n.kind == Kind::Window {
                ("main", "mcf-window")
            } else {
                ("div", "mcf-box")
            };
            let stack =
                matches!(n.style.get(&Style::Layout), Some(StyleValue::Word(w)) if w == "stack");
            let _ = write!(out, "<{tag} class=\"{class}\"{style_attr}>");
            for c in &n.children {
                node(c, Some(stack), out);
            }
            let _ = writeln!(out, "</{tag}>");
        }
        Kind::Text => {
            let _ = writeln!(out, "<span{style_attr}>{}</span>", text(Prop::Text));
        }
        Kind::Button => {
            let _ = writeln!(
                out,
                "<button type=\"button\" class=\"mcf-button\"{style_attr}>{}</button>",
                text(Prop::Label)
            );
        }
        Kind::Input => {
            let _ = writeln!(
                out,
                "<input class=\"mcf-input\" value=\"{}\" placeholder=\"{}\"{style_attr}>",
                text(Prop::Value),
                text(Prop::Placeholder)
            );
        }
        Kind::Image => {
            let _ = writeln!(
                out,
                "<div class=\"mcf-image\"{style_attr}><b>{}</b><span>{}</span></div>",
                text(Prop::Alt),
                text(Prop::Source)
            );
        }
    }
}

//! Page interactive de l'aperçu (`mcf view --serve`) : format JSON de l'arbre et script de la page.

use mcf_view::{Event, Node, Style, StyleValue};

use crate::html::{css, escape, BASE_CSS};

/// Arbre au format de la page : les styles sont déjà en CSS (construit à partir de valeurs
/// typées), le texte reste du texte pour la page.
pub fn live_json(n: &Node, parent_stack: bool) -> serde_json::Value {
    let stack = matches!(n.style.get(&Style::Layout), Some(StyleValue::Word(w)) if w == "stack");
    let props: serde_json::Map<String, serde_json::Value> = n
        .props
        .iter()
        .map(|(k, v)| (k.name().to_string(), serde_json::Value::String(v.clone())))
        .collect();
    let events: Vec<&str> = n
        .events
        .iter()
        .map(|e| match e {
            Event::Click => "click",
            Event::Enter => "enter",
            Event::Leave => "leave",
            Event::Change => "change",
            Event::Submit => "submit",
        })
        .collect();
    serde_json::json!({
        "kind": n.kind.name(),
        "props": props,
        "css": css(n, parent_stack),
        "events": events,
        "children": n.children.iter().map(|c| live_json(c, stack)).collect::<Vec<_>>(),
    })
}

/// Page interactive : récupère la vue (`/view`), la reporte dans le DOM par comparaison de
/// position et renvoie les événements (`/event`) au serveur local.
pub fn live_page(title: &str, key: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'\">\n\
<title>{}</title>\n<style>{BASE_CSS}\
#mcf-error{{position:fixed;left:0;right:0;bottom:0;background:#f38ba8;color:#11111b;padding:8px 16px;\
font:13px monospace;white-space:pre-wrap;display:none}}</style>\n</head>\n<body>\n\
<div id=\"mcf-root\"></div><div id=\"mcf-error\"></div>\n<script>const KEY=\"{key}\";\n{LIVE_JS}</script>\n</body>\n</html>\n",
        escape(title)
    )
}

const LIVE_JS: &str = r#"
const root = document.getElementById('mcf-root');
const errBox = document.getElementById('mcf-error');
const TAGS = {Window:'main', Box:'div', Text:'span', Image:'div', Button:'button', Input:'input'};
const CLASSES = {Window:'mcf-window', Box:'mcf-box', Button:'mcf-button', Input:'mcf-input', Image:'mcf-image'};
const CONTAINERS = new Set(['Window', 'Box']);

function pathOf(el) {
  const path = [];
  while (el && el !== root.firstElementChild) {
    const parent = el.parentElement;
    path.unshift(Array.prototype.indexOf.call(parent.children, el));
    el = parent;
  }
  return path;
}

async function send(el, event, text) {
  const body = JSON.stringify({path: pathOf(el), event, text});
  const r = await fetch('/event?k=' + KEY, {method: 'POST', body});
  await apply(r);
}

async function apply(r) {
  const data = await r.json();
  if (data.error) { errBox.textContent = data.error; errBox.style.display = 'block'; return; }
  errBox.style.display = 'none';
  if (data.idle) return;
  sync(root, 0, data.tree);
}

function create(n) {
  const el = document.createElement(TAGS[n.kind]);
  el.dataset.kind = n.kind;
  if (CLASSES[n.kind]) el.className = CLASSES[n.kind];
  if (n.kind === 'Button') el.type = 'button';
  if (n.kind === 'Image') { el.appendChild(document.createElement('b')); el.appendChild(document.createElement('span')); }
  el._events = new Set();
  el.addEventListener('click', e => { if (el._events.has('click')) { e.stopPropagation(); send(el, 'click'); } });
  el.addEventListener('mouseenter', () => { if (el._events.has('enter')) send(el, 'enter'); });
  el.addEventListener('mouseleave', () => { if (el._events.has('leave')) send(el, 'leave'); });
  if (n.kind === 'Input') {
    el.addEventListener('input', () => { if (el._events.has('change')) send(el, 'change', el.value); });
    el.addEventListener('keydown', e => { if (e.key === 'Enter' && el._events.has('submit')) send(el, 'submit', el.value); });
  }
  return el;
}

function update(el, n) {
  el.style.cssText = n.css;
  el._events = new Set(n.events);
  const p = n.props;
  if (n.kind === 'Text' || n.kind === 'Button') el.textContent = p.text ?? p.label ?? '';
  else if (n.kind === 'Input') {
    const v = p.value ?? '';
    if (el.value !== v) el.value = v;
    el.placeholder = p.placeholder ?? '';
  } else if (n.kind === 'Image') {
    el.children[0].textContent = p.alt ?? '';
    el.children[1].textContent = p.source ?? '';
  }
}

function sync(parent, i, n) {
  let el = parent.children[i];
  if (!el || el.dataset.kind !== n.kind) {
    const fresh = create(n);
    if (el) parent.replaceChild(fresh, el); else parent.appendChild(fresh);
    el = fresh;
  }
  update(el, n);
  if (CONTAINERS.has(n.kind)) {
    n.children.forEach((c, k) => sync(el, k, c));
    while (el.children.length > n.children.length) el.removeChild(el.lastElementChild);
  }
}

fetch('/view?k=' + KEY).then(apply);
// Les actions qui dorment (`sleep`) reprennent ici : le serveur répond `idle` s'il n'y a rien de neuf.
setInterval(() => { fetch('/tick?k=' + KEY).then(apply).catch(() => {}); }, 250);
"#;

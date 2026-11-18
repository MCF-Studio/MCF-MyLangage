//! Arbre de vue des interfaces `.uimcf` et changements à appliquer à l'affichage (phase 11, D66).
//!
//! Cette crate ne connaît ni le DOM ni Bevy : le runtime calcule un arbre ([`Node`]) à partir
//! de la vue racine, [`Renderer::update`] le compare au précédent et produit une liste de
//! [`Change`]. Chaque cible applique ces changements à son affichage (web : éléments du DOM,
//! D61 ; MCF Engine : entités `bevy_ui`). Règles : `docs/cibles/affichage.md` (D68).
//!
//! - Chaque élément affiché reçoit un [`NodeId`] à sa création ; il le garde tant qu'il reste
//!   à la même place, et un identifiant n'est jamais réutilisé.
//! - Identité **par position** (D61) : l'enfant n°`i` d'un élément est comparé à l'ancien
//!   enfant n°`i`. Même sorte d'élément → mis à jour en place ; sinon remplacé.
//! - Les valeurs sont déjà validées (couleurs analysées, longueurs typées) : une cible n'a
//!   jamais de texte à interpréter comme du HTML, du CSS ou un chemin.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

/// Nombre d'éléments maximal dans une vue (MCF0416, D61).
pub const MAX_NODES: usize = 10_000;

/// Élément de base (D53).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    Window,
    Box,
    Text,
    Image,
    Button,
    Input,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Window,
        Kind::Box,
        Kind::Text,
        Kind::Image,
        Kind::Button,
        Kind::Input,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Window => "Window",
            Kind::Box => "Box",
            Kind::Text => "Text",
            Kind::Image => "Image",
            Kind::Button => "Button",
            Kind::Input => "Input",
        }
    }

    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// `Window` et `Box` contiennent des enfants ; les autres non.
    pub fn has_children(self) -> bool {
        matches!(self, Kind::Window | Kind::Box)
    }
}

/// Propriété texte d'un élément (`title`, `text`, `label`…). Le texte est affiché tel quel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Prop {
    Title,
    Text,
    Source,
    Alt,
    Label,
    Value,
    Placeholder,
}

impl Prop {
    pub const ALL: [Prop; 7] = [
        Prop::Title,
        Prop::Text,
        Prop::Source,
        Prop::Alt,
        Prop::Label,
        Prop::Value,
        Prop::Placeholder,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Prop::Title => "title",
            Prop::Text => "text",
            Prop::Source => "source",
            Prop::Alt => "alt",
            Prop::Label => "label",
            Prop::Value => "value",
            Prop::Placeholder => "placeholder",
        }
    }

    pub fn from_name(name: &str) -> Option<Prop> {
        Prop::ALL.into_iter().find(|p| p.name() == name)
    }
}

/// Événement qu'un élément écoute (`onClick`…).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Event {
    Click,
    Enter,
    Leave,
    /// `Input` : à chaque modification du texte.
    Change,
    /// `Input` : touche Entrée.
    Submit,
}

impl Event {
    pub const ALL: [Event; 5] = [
        Event::Click,
        Event::Enter,
        Event::Leave,
        Event::Change,
        Event::Submit,
    ];

    /// Nom dans un `.uimcf` (`onClick`…).
    pub fn name(self) -> &'static str {
        match self {
            Event::Click => "onClick",
            Event::Enter => "onEnter",
            Event::Leave => "onLeave",
            Event::Change => "onChange",
            Event::Submit => "onSubmit",
        }
    }

    pub fn from_name(name: &str) -> Option<Event> {
        Event::ALL.into_iter().find(|e| e.name() == name)
    }
}

/// Propriété de style (liste fermée, D54).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Style {
    Width,
    Height,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    Padding,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    Margin,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    Gap,
    BorderWidth,
    Radius,
    FontSize,
    X,
    Y,
    Background,
    Color,
    BorderColor,
    FontWeight,
    Opacity,
    Grow,
    Visible,
    Layout,
    Align,
    Justify,
    TextAlign,
    Overflow,
}

impl Style {
    pub const ALL: [Style; 34] = [
        Style::Width,
        Style::Height,
        Style::MinWidth,
        Style::MaxWidth,
        Style::MinHeight,
        Style::MaxHeight,
        Style::Padding,
        Style::PaddingTop,
        Style::PaddingRight,
        Style::PaddingBottom,
        Style::PaddingLeft,
        Style::Margin,
        Style::MarginTop,
        Style::MarginRight,
        Style::MarginBottom,
        Style::MarginLeft,
        Style::Gap,
        Style::BorderWidth,
        Style::Radius,
        Style::FontSize,
        Style::X,
        Style::Y,
        Style::Background,
        Style::Color,
        Style::BorderColor,
        Style::FontWeight,
        Style::Opacity,
        Style::Grow,
        Style::Visible,
        Style::Layout,
        Style::Align,
        Style::Justify,
        Style::TextAlign,
        Style::Overflow,
    ];

    /// Nom dans un `.uimcf`.
    pub fn name(self) -> &'static str {
        match self {
            Style::Width => "width",
            Style::Height => "height",
            Style::MinWidth => "minWidth",
            Style::MaxWidth => "maxWidth",
            Style::MinHeight => "minHeight",
            Style::MaxHeight => "maxHeight",
            Style::Padding => "padding",
            Style::PaddingTop => "paddingTop",
            Style::PaddingRight => "paddingRight",
            Style::PaddingBottom => "paddingBottom",
            Style::PaddingLeft => "paddingLeft",
            Style::Margin => "margin",
            Style::MarginTop => "marginTop",
            Style::MarginRight => "marginRight",
            Style::MarginBottom => "marginBottom",
            Style::MarginLeft => "marginLeft",
            Style::Gap => "gap",
            Style::BorderWidth => "borderWidth",
            Style::Radius => "radius",
            Style::FontSize => "fontSize",
            Style::X => "x",
            Style::Y => "y",
            Style::Background => "background",
            Style::Color => "color",
            Style::BorderColor => "borderColor",
            Style::FontWeight => "fontWeight",
            Style::Opacity => "opacity",
            Style::Grow => "grow",
            Style::Visible => "visible",
            Style::Layout => "layout",
            Style::Align => "align",
            Style::Justify => "justify",
            Style::TextAlign => "textAlign",
            Style::Overflow => "overflow",
        }
    }

    pub fn from_name(name: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// Longueur : pixels ou pourcentage du parent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Length {
    Px(i64),
    Percent(f64),
}

/// Couleur RVBA, déjà analysée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// `#rgb`, `#rgba`, `#rrggbb` ou `#rrggbbaa` (D61) ; toute autre chaîne → `None`
    /// (MCF0415 à l'exécution, jamais une couleur inventée).
    pub fn parse(text: &str) -> Option<Color> {
        let hex = text.strip_prefix('#')?;
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok();
        let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        match hex.len() {
            3 | 4 => {
                let short = |i: usize| digit(i).map(|d| d * 17);
                Some(Color {
                    r: short(0)?,
                    g: short(1)?,
                    b: short(2)?,
                    a: if hex.len() == 4 { short(3)? } else { 255 },
                })
            }
            6 | 8 => Some(Color {
                r: pair(0)?,
                g: pair(2)?,
                b: pair(4)?,
                a: if hex.len() == 8 { pair(6)? } else { 255 },
            }),
            _ => None,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02x}", self.a)?;
        }
        Ok(())
    }
}

/// Valeur d'une propriété de style, déjà vérifiée (plages D54).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleValue {
    Length(Length),
    Color(Color),
    /// `fontWeight` : 100 à 900.
    Weight(u16),
    /// `opacity` : 0 à 1.
    Opacity(f64),
    /// `grow` : entier positif.
    Grow(i64),
    Bool(bool),
    /// `layout`, `align`… : un des mots permis pour la propriété.
    Word(String),
}

/// Élément calculé par le runtime à partir de la vue racine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<Prop, String>,
    /// Style final : style nommé, puis propriétés de l'élément (D54).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub style: BTreeMap<Style, StyleValue>,
    /// Événements écoutés. L'action associée reste dans le runtime.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub events: BTreeSet<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(kind: Kind) -> Node {
        Node {
            kind,
            props: BTreeMap::new(),
            style: BTreeMap::new(),
            events: BTreeSet::new(),
            children: Vec::new(),
        }
    }

    pub fn prop(mut self, prop: Prop, value: impl Into<String>) -> Node {
        self.props.insert(prop, value.into());
        self
    }

    pub fn style(mut self, style: Style, value: StyleValue) -> Node {
        self.style.insert(style, value);
        self
    }

    pub fn on(mut self, event: Event) -> Node {
        self.events.insert(event);
        self
    }

    pub fn child(mut self, child: Node) -> Node {
        self.children.push(child);
        self
    }

    /// Nombre d'éléments, celui-ci compris.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }

    /// Rang de l'élément désigné par `path` (indices d'enfants depuis cet élément) dans le
    /// parcours « parent avant enfants » ; `None` si le chemin sort de l'arbre. Le runtime
    /// numérote ses éléments dans cet ordre : c'est ainsi qu'un événement retrouve son action.
    pub fn preorder_index(&self, path: &[usize]) -> Option<usize> {
        let mut index = 0;
        let mut node = self;
        for &i in path {
            let target = node.children.get(i)?;
            index += 1 + node.children[..i].iter().map(Node::count).sum::<usize>();
            node = target;
        }
        Some(index)
    }
}

/// Identifiant d'un élément affiché, attribué à sa création, jamais réutilisé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

/// Changement à appliquer à l'affichage, dans l'ordre de la liste.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Change {
    /// Crée un élément vide (sans propriété, style, événement ni enfant) et l'insère à la
    /// position `index` parmi les enfants actuels de `parent` (`None` : racine). Ses
    /// propriétés et ses enfants suivent dans la liste.
    Create {
        id: NodeId,
        parent: Option<NodeId>,
        index: usize,
        kind: Kind,
    },
    /// Retire l'élément et tous ses descendants, listés dans `subtree` (lui compris, parents
    /// avant enfants) pour que la cible oublie leurs identifiants.
    Remove {
        id: NodeId,
        subtree: Vec<NodeId>,
    },
    SetProp {
        id: NodeId,
        prop: Prop,
        value: String,
    },
    RemoveProp {
        id: NodeId,
        prop: Prop,
    },
    SetStyle {
        id: NodeId,
        style: Style,
        value: StyleValue,
    },
    /// La propriété revient à sa valeur par défaut.
    RemoveStyle {
        id: NodeId,
        style: Style,
    },
    Listen {
        id: NodeId,
        event: Event,
    },
    Unlisten {
        id: NodeId,
        event: Event,
    },
}

/// Vue trop grande (MCF0416).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooManyNodes {
    pub count: usize,
}

impl fmt::Display for TooManyNodes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "la vue contient {} éléments, au-delà de la limite de {MAX_NODES}",
            self.count
        )
    }
}

impl std::error::Error for TooManyNodes {}

/// Élément affiché : son identifiant et ce qu'il montre.
#[derive(Clone, Debug)]
struct Mounted {
    id: NodeId,
    node: Node,
    children: Vec<Mounted>,
}

/// Garde la vue affichée et calcule les changements vers la suivante.
#[derive(Debug, Default)]
pub struct Renderer {
    next_id: u64,
    root: Option<Mounted>,
    /// Position de chaque élément affiché (indices d'enfants depuis la racine).
    paths: HashMap<NodeId, Vec<usize>>,
}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer::default()
    }

    /// Remplace la vue affichée par `view` (`None` : plus rien) et rend les changements à
    /// appliquer. Si la vue dépasse [`MAX_NODES`], rien ne change et l'erreur est rendue.
    pub fn update(&mut self, view: Option<Node>) -> Result<Vec<Change>, TooManyNodes> {
        if let Some(node) = &view {
            let count = node.count();
            if count > MAX_NODES {
                return Err(TooManyNodes { count });
            }
        }
        let mut out = Vec::new();
        self.root = match (self.root.take(), view) {
            (None, None) => None,
            (None, Some(new)) => Some(self.create(None, 0, new, &mut out)),
            (Some(old), None) => {
                remove(&old, &mut out);
                None
            }
            (Some(old), Some(new)) => Some(self.reconcile(None, 0, old, new, &mut out)),
        };
        self.paths.clear();
        if let Some(root) = &self.root {
            index_paths(root, &mut Vec::new(), &mut self.paths);
        }
        Ok(out)
    }

    /// Position actuelle d'un élément affiché (indices d'enfants depuis la racine), pour
    /// retrouver l'action d'un événement reçu de la cible. `None` : élément qui n'est plus
    /// affiché (événement arrivé après un changement : à ignorer).
    pub fn path(&self, id: NodeId) -> Option<&[usize]> {
        self.paths.get(&id).map(Vec::as_slice)
    }

    /// Identifiant de la racine affichée.
    pub fn root(&self) -> Option<NodeId> {
        self.root.as_ref().map(|m| m.id)
    }

    fn create(
        &mut self,
        parent: Option<NodeId>,
        index: usize,
        node: Node,
        out: &mut Vec<Change>,
    ) -> Mounted {
        self.next_id += 1;
        let id = NodeId(self.next_id);
        out.push(Change::Create {
            id,
            parent,
            index,
            kind: node.kind,
        });
        for (&prop, value) in &node.props {
            out.push(Change::SetProp {
                id,
                prop,
                value: value.clone(),
            });
        }
        for (&style, value) in &node.style {
            out.push(Change::SetStyle {
                id,
                style,
                value: value.clone(),
            });
        }
        for &event in &node.events {
            out.push(Change::Listen { id, event });
        }
        let mut node = node;
        let children = std::mem::take(&mut node.children)
            .into_iter()
            .enumerate()
            .map(|(i, child)| self.create(Some(id), i, child, out))
            .collect();
        Mounted { id, node, children }
    }

    fn reconcile(
        &mut self,
        parent: Option<NodeId>,
        index: usize,
        old: Mounted,
        new: Node,
        out: &mut Vec<Change>,
    ) -> Mounted {
        if old.node.kind != new.kind {
            remove(&old, out);
            return self.create(parent, index, new, out);
        }
        let id = old.id;
        diff_map(
            &old.node.props,
            &new.props,
            out,
            |prop, value| match value {
                Some(value) => Change::SetProp { id, prop, value },
                None => Change::RemoveProp { id, prop },
            },
        );
        diff_map(
            &old.node.style,
            &new.style,
            out,
            |style, value| match value {
                Some(value) => Change::SetStyle { id, style, value },
                None => Change::RemoveStyle { id, style },
            },
        );
        for &event in old.node.events.difference(&new.events) {
            out.push(Change::Unlisten { id, event });
        }
        for &event in new.events.difference(&old.node.events) {
            out.push(Change::Listen { id, event });
        }

        let mut new = new;
        let new_children = std::mem::take(&mut new.children);
        let mut old_children = old.children;
        // Enfants en trop retirés en partant de la fin : les positions des autres ne bougent pas.
        let kept = new_children.len().min(old_children.len());
        for extra in old_children.drain(kept..).rev() {
            remove(&extra, out);
        }
        let mut children = Vec::with_capacity(new_children.len());
        let mut old_children = old_children.into_iter();
        for (i, child) in new_children.into_iter().enumerate() {
            children.push(match old_children.next() {
                Some(old_child) => self.reconcile(Some(id), i, old_child, child, out),
                None => self.create(Some(id), i, child, out),
            });
        }
        Mounted {
            id,
            node: new,
            children,
        }
    }
}

/// Changements d'une table de valeurs (propriétés ou style) : retraits, puis ajouts et
/// modifications, dans l'ordre des clés.
fn diff_map<K: Copy + Ord, V: Clone + PartialEq>(
    old: &BTreeMap<K, V>,
    new: &BTreeMap<K, V>,
    out: &mut Vec<Change>,
    change: impl Fn(K, Option<V>) -> Change,
) {
    for key in old.keys().filter(|k| !new.contains_key(k)) {
        out.push(change(*key, None));
    }
    for (key, value) in new {
        if old.get(key) != Some(value) {
            out.push(change(*key, Some(value.clone())));
        }
    }
}

fn remove(old: &Mounted, out: &mut Vec<Change>) {
    let mut subtree = Vec::new();
    collect_ids(old, &mut subtree);
    out.push(Change::Remove {
        id: old.id,
        subtree,
    });
}

fn collect_ids(m: &Mounted, out: &mut Vec<NodeId>) {
    out.push(m.id);
    for child in &m.children {
        collect_ids(child, out);
    }
}

fn index_paths(m: &Mounted, path: &mut Vec<usize>, out: &mut HashMap<NodeId, Vec<usize>>) {
    out.insert(m.id, path.clone());
    for (i, child) in m.children.iter().enumerate() {
        path.push(i);
        index_paths(child, path, out);
        path.pop();
    }
}

/// Erreur d'application : liste de changements incohérente avec l'affichage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyError(pub String);

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "changement inapplicable : {}", self.0)
    }
}

impl std::error::Error for ApplyError {}

/// Affichage de référence, sans écran : applique des [`Change`] à un arbre en mémoire.
/// Sert aux tests du runtime et des cibles (le résultat doit être identique à la vue
/// calculée) et documente le sens exact de chaque changement.
#[derive(Clone, Debug, Default)]
pub struct Document {
    nodes: HashMap<NodeId, DocNode>,
    root: Option<NodeId>,
}

#[derive(Clone, Debug)]
struct DocNode {
    node: Node,
    children: Vec<NodeId>,
    parent: Option<NodeId>,
}

impl Document {
    pub fn new() -> Document {
        Document::default()
    }

    pub fn apply(&mut self, changes: &[Change]) -> Result<(), ApplyError> {
        changes.iter().try_for_each(|c| self.apply_one(c))
    }

    fn get(&mut self, id: NodeId) -> Result<&mut DocNode, ApplyError> {
        self.nodes
            .get_mut(&id)
            .ok_or_else(|| ApplyError(format!("élément {} inconnu", id.0)))
    }

    fn apply_one(&mut self, change: &Change) -> Result<(), ApplyError> {
        match change {
            Change::Create {
                id,
                parent,
                index,
                kind,
            } => {
                if self.nodes.contains_key(id) {
                    return Err(ApplyError(format!("élément {} déjà créé", id.0)));
                }
                match parent {
                    None if self.root.is_some() => {
                        return Err(ApplyError("la racine existe déjà".into()))
                    }
                    None => self.root = Some(*id),
                    Some(p) => {
                        let parent = self.get(*p)?;
                        if !parent.node.kind.has_children() {
                            return Err(ApplyError(format!(
                                "`{}` ne contient pas d'enfants",
                                parent.node.kind.name()
                            )));
                        }
                        if *index > parent.children.len() {
                            return Err(ApplyError(format!("position {index} hors du parent")));
                        }
                        parent.children.insert(*index, *id);
                    }
                }
                self.nodes.insert(
                    *id,
                    DocNode {
                        node: Node::new(*kind),
                        children: Vec::new(),
                        parent: *parent,
                    },
                );
            }
            Change::Remove { id, subtree } => {
                let parent = self.get(*id)?.parent;
                match parent {
                    None => self.root = None,
                    Some(p) => self.get(p)?.children.retain(|c| c != id),
                }
                let mut actual = Vec::new();
                self.collect(*id, &mut actual);
                if &actual != subtree {
                    return Err(ApplyError(format!(
                        "descendants de {} mal listés : {subtree:?} au lieu de {actual:?}",
                        id.0
                    )));
                }
                for n in actual {
                    self.nodes.remove(&n);
                }
            }
            Change::SetProp { id, prop, value } => {
                self.get(*id)?.node.props.insert(*prop, value.clone());
            }
            Change::RemoveProp { id, prop } => {
                self.get(*id)?.node.props.remove(prop);
            }
            Change::SetStyle { id, style, value } => {
                self.get(*id)?.node.style.insert(*style, value.clone());
            }
            Change::RemoveStyle { id, style } => {
                self.get(*id)?.node.style.remove(style);
            }
            Change::Listen { id, event } => {
                self.get(*id)?.node.events.insert(*event);
            }
            Change::Unlisten { id, event } => {
                self.get(*id)?.node.events.remove(event);
            }
        }
        Ok(())
    }

    fn collect(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        if let Some(n) = self.nodes.get(&id) {
            for &c in &n.children {
                self.collect(c, out);
            }
        }
    }

    /// Arbre affiché, pour le comparer à la vue calculée.
    pub fn tree(&self) -> Option<Node> {
        self.root.map(|r| self.build(r))
    }

    fn build(&self, id: NodeId) -> Node {
        let n = &self.nodes[&id];
        let mut node = n.node.clone();
        node.children = n.children.iter().map(|&c| self.build(c)).collect();
        node
    }

    /// Nombre d'éléments affichés.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests;

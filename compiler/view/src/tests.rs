use super::*;

fn px(n: i64) -> StyleValue {
    StyleValue::Length(Length::Px(n))
}

fn text(t: &str) -> Node {
    Node::new(Kind::Text).prop(Prop::Text, t)
}

/// Applique la mise à jour au document de référence et vérifie qu'il montre exactement la vue.
fn step(r: &mut Renderer, doc: &mut Document, view: Option<Node>) -> Vec<Change> {
    let changes = r.update(view.clone()).expect("vue de taille raisonnable");
    doc.apply(&changes).expect("changements applicables");
    assert_eq!(doc.tree(), view);
    if let Some(v) = &view {
        assert_eq!(doc.len(), v.count());
    } else {
        assert!(doc.is_empty());
    }
    changes
}

#[test]
fn premier_affichage_cree_tout_dans_l_ordre() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    let view = Node::new(Kind::Window)
        .prop(Prop::Title, "Compteur")
        .child(text("0"))
        .child(
            Node::new(Kind::Button)
                .prop(Prop::Label, "+1")
                .on(Event::Click),
        );
    let changes = step(&mut r, &mut doc, Some(view));
    assert_eq!(
        changes[0],
        Change::Create {
            id: NodeId(1),
            parent: None,
            index: 0,
            kind: Kind::Window
        }
    );
    assert!(changes.contains(&Change::Listen {
        id: NodeId(3),
        event: Event::Click
    }));
    assert_eq!(r.root(), Some(NodeId(1)));
    assert_eq!(r.path(NodeId(3)), Some(&[1][..]));
}

#[test]
fn vue_identique_aucun_changement() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    let view = Node::new(Kind::Box)
        .style(Style::Padding, px(8))
        .child(text("a"));
    step(&mut r, &mut doc, Some(view.clone()));
    assert!(step(&mut r, &mut doc, Some(view)).is_empty());
}

#[test]
fn texte_modifie_en_place() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    step(
        &mut r,
        &mut doc,
        Some(Node::new(Kind::Box).child(text("0"))),
    );
    let changes = step(
        &mut r,
        &mut doc,
        Some(Node::new(Kind::Box).child(text("1"))),
    );
    assert_eq!(
        changes,
        vec![Change::SetProp {
            id: NodeId(2),
            prop: Prop::Text,
            value: "1".into()
        }]
    );
}

#[test]
fn if_qui_bascule_remplace_l_element() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    step(
        &mut r,
        &mut doc,
        Some(Node::new(Kind::Box).child(text("connecté"))),
    );
    let changes = step(
        &mut r,
        &mut doc,
        Some(Node::new(Kind::Box).child(Node::new(Kind::Button).prop(Prop::Label, "Connexion"))),
    );
    assert_eq!(
        changes[0],
        Change::Remove {
            id: NodeId(2),
            subtree: vec![NodeId(2)]
        }
    );
    assert!(matches!(
        changes[1],
        Change::Create {
            id: NodeId(3),
            parent: Some(NodeId(1)),
            index: 0,
            kind: Kind::Button
        }
    ));
    assert_eq!(
        r.path(NodeId(2)),
        None,
        "un élément retiré n'a plus de position"
    );
}

#[test]
fn foreach_qui_grandit_puis_retrecit() {
    let list = |n: usize| (0..n).fold(Node::new(Kind::Box), |b, i| b.child(text(&i.to_string())));
    let mut r = Renderer::new();
    let mut doc = Document::new();
    step(&mut r, &mut doc, Some(list(2)));
    let grow = step(&mut r, &mut doc, Some(list(4)));
    assert_eq!(
        grow.iter()
            .filter(|c| matches!(c, Change::Create { .. }))
            .count(),
        2
    );
    let shrink = step(&mut r, &mut doc, Some(list(1)));
    // Retraits depuis la fin : les éléments restants gardent leur position.
    let removed: Vec<_> = shrink
        .iter()
        .filter_map(|c| match c {
            Change::Remove { id, .. } => Some(id.0),
            _ => None,
        })
        .collect();
    assert_eq!(removed, vec![5, 4, 3]);
    assert_eq!(r.path(NodeId(2)), Some(&[0][..]));
}

#[test]
fn styles_et_evenements_ajoutes_et_retires() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    let a = Node::new(Kind::Button)
        .prop(Prop::Label, "ok")
        .style(
            Style::Background,
            StyleValue::Color(Color::parse("#112233").unwrap()),
        )
        .on(Event::Click);
    let b = Node::new(Kind::Button)
        .prop(Prop::Label, "ok")
        .style(Style::Radius, px(4))
        .on(Event::Enter);
    let changes = step(&mut r, &mut doc, Some(a));
    assert!(!changes.is_empty());
    let changes = step(&mut r, &mut doc, Some(b));
    assert_eq!(
        changes,
        vec![
            Change::RemoveStyle {
                id: NodeId(1),
                style: Style::Background
            },
            Change::SetStyle {
                id: NodeId(1),
                style: Style::Radius,
                value: px(4)
            },
            Change::Unlisten {
                id: NodeId(1),
                event: Event::Click
            },
            Change::Listen {
                id: NodeId(1),
                event: Event::Enter
            },
        ]
    );
}

#[test]
fn retrait_liste_tout_le_sous_arbre() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    let view = Node::new(Kind::Box).child(Node::new(Kind::Box).child(text("a")).child(text("b")));
    step(&mut r, &mut doc, Some(view));
    let changes = step(&mut r, &mut doc, None);
    assert_eq!(
        changes,
        vec![Change::Remove {
            id: NodeId(1),
            subtree: vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)]
        }]
    );
    assert_eq!(r.root(), None);
}

#[test]
fn trop_d_elements_refuse_sans_rien_changer() {
    let mut r = Renderer::new();
    let mut doc = Document::new();
    step(&mut r, &mut doc, Some(text("avant")));
    let big = (0..MAX_NODES).fold(Node::new(Kind::Box), |b, _| b.child(text("x")));
    assert_eq!(
        r.update(Some(big)),
        Err(TooManyNodes {
            count: MAX_NODES + 1
        })
    );
    // L'affichage précédent est toujours celui que connaît le Renderer.
    assert!(step(&mut r, &mut doc, Some(text("avant"))).is_empty());
}

#[test]
fn couleurs() {
    let c = |s: &str| Color::parse(s);
    assert_eq!(
        c("#fff"),
        Some(Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255
        })
    );
    assert_eq!(
        c("#1e1e2e80"),
        Some(Color {
            r: 0x1e,
            g: 0x1e,
            b: 0x2e,
            a: 0x80
        })
    );
    assert_eq!(c("#0f08").map(|c| c.a), Some(0x88));
    for bad in [
        "", "#", "fff", "#ff", "#fffff", "#ggg", "#+1f", "red", "#ffé", "#fff;x",
    ] {
        assert_eq!(c(bad), None, "{bad:?}");
    }
    assert_eq!(c("#AbCdEf").unwrap().to_string(), "#abcdef");
    assert_eq!(c("#abcdef01").unwrap().to_string(), "#abcdef01");
}

#[test]
fn noms_aller_retour() {
    for k in Kind::ALL {
        assert_eq!(Kind::from_name(k.name()), Some(k));
    }
    for p in Prop::ALL {
        assert_eq!(Prop::from_name(p.name()), Some(p));
    }
    for e in Event::ALL {
        assert_eq!(Event::from_name(e.name()), Some(e));
    }
    for s in Style::ALL {
        assert_eq!(Style::from_name(s.name()), Some(s));
    }
    assert_eq!(Style::from_name("onClick"), None);
}

#[test]
fn format_json_des_changements() {
    let change = Change::SetStyle {
        id: NodeId(7),
        style: Style::FontSize,
        value: StyleValue::Length(Length::Percent(50.0)),
    };
    let json = serde_json::to_string(&change).unwrap();
    assert_eq!(
        json,
        r#"{"op":"setStyle","id":7,"style":"fontSize","value":{"length":{"percent":50.0}}}"#
    );
    assert_eq!(serde_json::from_str::<Change>(&json).unwrap(), change);
}

#[test]
fn changements_incoherents_refuses() {
    let mut doc = Document::new();
    let create = |id, parent: Option<u64>, kind| Change::Create {
        id: NodeId(id),
        parent: parent.map(NodeId),
        index: 0,
        kind,
    };
    assert!(doc
        .apply(&[create(1, None, Kind::Text), create(2, Some(1), Kind::Text)])
        .is_err());
    let mut doc = Document::new();
    assert!(doc
        .apply(&[create(1, None, Kind::Box), create(1, Some(1), Kind::Text)])
        .is_err());
    let mut doc = Document::new();
    assert!(doc
        .apply(&[Change::Remove {
            id: NodeId(9),
            subtree: vec![NodeId(9)]
        }])
        .is_err());
}

/// Générateur pseudo-aléatoire déterministe (pas de dépendance externe).
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn random_node(g: &mut Lcg, depth: u32) -> Node {
    let kind = Kind::ALL[g.below(6) as usize];
    let mut node = Node::new(kind);
    for _ in 0..g.below(3) {
        node = node.prop(Prop::ALL[g.below(7) as usize], g.below(3).to_string());
    }
    for _ in 0..g.below(3) {
        node = node.style(Style::ALL[g.below(6) as usize], px(g.below(3) as i64));
    }
    for _ in 0..g.below(2) {
        node = node.on(Event::ALL[g.below(5) as usize]);
    }
    if kind.has_children() && depth > 0 {
        for _ in 0..g.below(4) {
            node = node.child(random_node(g, depth - 1));
        }
    }
    node
}

#[test]
fn suites_aleatoires_de_vues() {
    let mut g = Lcg(42);
    for _ in 0..200 {
        let mut r = Renderer::new();
        let mut doc = Document::new();
        for _ in 0..8 {
            let view = if g.below(10) == 0 {
                None
            } else {
                Some(random_node(&mut g, 4))
            };
            step(&mut r, &mut doc, view.clone());
            // Chaque élément affiché est retrouvé à sa position.
            if let Some(v) = &view {
                let mut ids = Vec::new();
                if let Some(root) = &r.root {
                    collect_ids(root, &mut ids);
                }
                assert_eq!(ids.len(), v.count());
                for id in ids {
                    let path = r.path(id).expect("position connue");
                    let mut at = v;
                    for &i in path {
                        at = &at.children[i];
                    }
                    assert_eq!(at.kind, find(r.root.as_ref().unwrap(), id).node.kind);
                }
            }
            // Recalculer la même vue ne change rien.
            assert!(r.update(view).unwrap().is_empty());
        }
    }
}

fn find(m: &Mounted, id: NodeId) -> &Mounted {
    if m.id == id {
        return m;
    }
    m.children
        .iter()
        .find_map(|c| {
            let mut ids = Vec::new();
            collect_ids(c, &mut ids);
            ids.contains(&id).then(|| find(c, id))
        })
        .expect("élément présent")
}

#[test]
fn preorder_index_follows_parent_then_children_order() {
    // Box
    //  ├─ Text a            (1)
    //  ├─ Box               (2)
    //  │   ├─ Text b        (3)
    //  │   └─ Text c        (4)
    //  └─ Text d            (5)
    let tree = Node::new(Kind::Box)
        .child(text("a"))
        .child(Node::new(Kind::Box).child(text("b")).child(text("c")))
        .child(text("d"));
    assert_eq!(tree.preorder_index(&[]), Some(0));
    assert_eq!(tree.preorder_index(&[0]), Some(1));
    assert_eq!(tree.preorder_index(&[1]), Some(2));
    assert_eq!(tree.preorder_index(&[1, 0]), Some(3));
    assert_eq!(tree.preorder_index(&[1, 1]), Some(4));
    assert_eq!(tree.preorder_index(&[2]), Some(5));
    // Chemins qui sortent de l'arbre.
    assert_eq!(tree.preorder_index(&[3]), None);
    assert_eq!(tree.preorder_index(&[0, 0]), None);
    // Le rang est celui de la position dans le parcours.
    assert_eq!(tree.count(), 6);
}

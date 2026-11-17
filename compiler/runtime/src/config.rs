//! Fichiers `.cfg` à l'exécution : relecture des déclarations `config` au démarrage et
//! `loadConfig` (DECISIONS D47–D51).
//!
//! Le fichier a pu changer depuis `mcf check` : il est donc relu et revalidé avec le même
//! code (`mcf_config`) que la vérification. Un fichier absent ou non conforme arrête le
//! programme (MCF0413) avec la première erreur trouvée dans le `.cfg`.

use std::collections::HashMap;
use std::sync::Arc;

use mcf_config::{Json, JsonKind, Schema, SchemaRef};
use mcf_diagnostics::SourceFile;

use mcf_syntax::ast::{TypeDecl, TypeRef};

use crate::value::{StructValue, Value, ValueCell};
use crate::{Entity, Program};

impl<'a> Program<'a> {
    /// Type d'une structure, d'une énumération : cherché depuis `table` puis les imports de
    /// `file`, sans les variables locales (un type n'en est jamais une).
    fn type_entity(&self, r: &TypeRef, table: usize, file: usize) -> Option<Entity> {
        let first = &r.path.segments[0].name;
        let mut entity = None;
        let mut t = Some(table);
        while let Some(id) = t {
            if let Some(e) = self.tables[id].names.get(first) {
                entity = Some(*e);
                break;
            }
            t = self.tables[id].parent;
        }
        if entity.is_none() {
            let (t, n) = self.imports[file].get(first)?;
            entity = self.tables[*t].names.get(n).copied();
        }
        let mut entity = entity?;
        for seg in &r.path.segments[1..] {
            let Entity::Scope(t) = entity else {
                return None;
            };
            entity = *self.tables[t].names.get(&seg.name)?;
        }
        Some(entity)
    }

    /// Schéma du type déclaré ; `None` seulement si la vérification a été contournée.
    pub(crate) fn schema_of(
        &self,
        decl: &TypeDecl,
        table: usize,
        file: usize,
    ) -> Option<SchemaRef> {
        self.schema_decl(decl, table, file, &mut HashMap::new())
    }

    fn schema_decl(
        &self,
        decl: &TypeDecl,
        table: usize,
        file: usize,
        memo: &mut HashMap<usize, SchemaRef>,
    ) -> Option<SchemaRef> {
        match decl.types.as_slice() {
            [one] => self.schema_ref(one, table, file, memo),
            [a, b] => Some(Arc::new(Schema::Dual(
                self.schema_ref(a, table, file, memo)?,
                self.schema_ref(b, table, file, memo)?,
            ))),
            _ => None,
        }
    }

    fn schema_ref(
        &self,
        r: &TypeRef,
        table: usize,
        file: usize,
        memo: &mut HashMap<usize, SchemaRef>,
    ) -> Option<SchemaRef> {
        if let [seg] = r.path.segments.as_slice() {
            let simple = match seg.name.as_str() {
                "Bool" => Some(Schema::Bool),
                "Int" => Some(Schema::Int),
                "Float" => Some(Schema::Float),
                "String" => Some(Schema::String),
                "Array" => match r.args.as_slice() {
                    [item] => Some(Schema::Array(self.schema_ref(item, table, file, memo)?)),
                    _ => return None,
                },
                "Map" => match r.args.as_slice() {
                    [_, v] => Some(Schema::Map(self.schema_ref(v, table, file, memo)?)),
                    _ => return None,
                },
                _ => None,
            };
            if let Some(s) = simple {
                return Some(Arc::new(s));
            }
        }
        match self.type_entity(r, table, file)? {
            Entity::Enum(id) => {
                let e = &self.types.enums[id];
                Some(Arc::new(Schema::Enum {
                    id,
                    name: e.name.clone(),
                    variants: e.variants.clone(),
                }))
            }
            Entity::Struct(id) => {
                if let Some(s) = memo.get(&id) {
                    return Some(s.clone());
                }
                let (decl, s_table, s_file) = self.struct_decls[id]?;
                let mut fields = Vec::new();
                for f in &decl.fields {
                    fields.push((
                        f.name.name.clone(),
                        self.schema_decl(&f.ty, s_table, s_file, memo)?,
                    ));
                }
                let s = Arc::new(Schema::Struct {
                    id,
                    name: decl.name.name.clone(),
                    fields,
                });
                memo.insert(id, s.clone());
                Some(s)
            }
            _ => None,
        }
    }

    /// Lit `relative` (depuis le dossier du fichier `file`), le valide et le convertit.
    /// `frozen` : valeur d'une déclaration `config`, qui se lit seulement.
    pub(crate) fn load_config(
        &self,
        file: usize,
        relative: &str,
        schema: &Schema,
        frozen: bool,
    ) -> Result<Value, String> {
        mcf_config::check_path(relative)?;
        let path = self.dirs[file].join(relative);
        let shown = path.display().to_string().replace('\\', "/");
        // Le chemin réel (`..` et liens symboliques résolus) doit rester dans le projet : un
        // chemin calculé à partir d'une donnée ne doit pas permettre de lire ailleurs (D51).
        let path = std::fs::canonicalize(&path)
            .map_err(|e| format!("impossible de lire `{shown}` : {e}"))?;
        let root = if self.root.as_os_str().is_empty() {
            std::path::Path::new(".")
        } else {
            self.root.as_path()
        };
        let root = std::fs::canonicalize(root)
            .map_err(|e| format!("dossier du projet illisible : {e}"))?;
        if !path.starts_with(&root) {
            return Err(format!(
                "`{shown}` est en dehors du dossier du projet : seuls les fichiers `.cfg` du projet se lisent"
            ));
        }
        let size = std::fs::metadata(&path)
            .map_err(|e| format!("impossible de lire `{shown}` : {e}"))?
            .len();
        if size > mcf_config::MAX_SIZE as u64 {
            return Err(format!(
                "`{shown}` fait {size} octets : la limite est de {} Mio",
                mcf_config::MAX_SIZE / (1024 * 1024)
            ));
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("impossible de lire `{shown}` : {e}"))?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        match mcf_config::load(text, schema) {
            Ok(json) => Ok(to_value(&json, schema, frozen)),
            Err(diags) => {
                let cfg = SourceFile::new(shown.clone(), text);
                let first = &diags[0];
                let at = cfg.location(first.span.start);
                let more = match diags.len() {
                    1 => String::new(),
                    n => format!(
                        " (et {} autre(s) problème(s) : `mcf check` les liste)",
                        n - 1
                    ),
                };
                Err(format!(
                    "`{shown}` n'est pas conforme au type attendu — {shown}:{}:{} : erreur[{}] {}{more}",
                    at.line, at.column, first.code.id, first.message
                ))
            }
        }
    }
}

/// Conversion d'une valeur déjà validée par `schema`.
fn to_value(v: &Json, schema: &Schema, frozen: bool) -> Value {
    match (schema, &v.kind) {
        (Schema::Dual(a, b), _) => match mcf_config::pick_member(v, a, b) {
            Some(member) => to_value(v, member, frozen),
            None => Value::Void,
        },
        (Schema::Bool, JsonKind::Bool(b)) => Value::Bool(*b),
        (Schema::Int, JsonKind::Number { text, .. }) => Value::Int(text.parse().unwrap_or(0)),
        (Schema::Float, JsonKind::Number { text, .. }) => {
            Value::Float(text.parse().unwrap_or(f64::NAN))
        }
        (Schema::String, JsonKind::Str(s)) => Value::Str(s.as_str().into()),
        (Schema::Enum { id, variants, .. }, JsonKind::Str(s)) => Value::Enum {
            id: *id,
            variant: variants.iter().position(|x| x == s).unwrap_or(0),
        },
        (Schema::Struct { id, fields, .. }, JsonKind::Object(members)) => {
            let values = fields
                .iter()
                .map(|(name, ty)| {
                    members
                        .iter()
                        .find(|m| m.key == *name)
                        .map_or(Value::Void, |m| to_value(&m.value, ty, frozen))
                })
                .collect();
            Value::Struct(Arc::new(cell(
                StructValue {
                    id: *id,
                    fields: values,
                },
                frozen,
            )))
        }
        (Schema::Map(ty), JsonKind::Object(members)) => {
            let entries: Vec<(Value, Value)> = members
                .iter()
                .map(|m| {
                    (
                        Value::Str(m.key.as_str().into()),
                        to_value(&m.value, ty, frozen),
                    )
                })
                .collect();
            Value::Map(Arc::new(cell(entries, frozen)))
        }
        (Schema::Array(ty), JsonKind::Array(items)) => {
            let values: Vec<Value> = items.iter().map(|i| to_value(i, ty, frozen)).collect();
            Value::Array(Arc::new(cell(values, frozen)))
        }
        // Impossible après `validate` : la valeur a été refusée avant d'arriver ici.
        _ => Value::Void,
    }
}

fn cell<T>(value: T, frozen: bool) -> ValueCell<T> {
    if frozen {
        ValueCell::new_frozen(value)
    } else {
        ValueCell::new(value)
    }
}

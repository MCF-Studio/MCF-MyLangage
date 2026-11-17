//! Exécution des programmes MCF (phase 6) : interpréteur qui parcourt l'AST.
//!
//! Le projet doit avoir été accepté par `mcf_semantic` (aucune erreur) : l'interpréteur ne
//! refait pas la vérification des types. Une incohérence qu'il rencontre malgré tout est un
//! bug du compilateur, signalée par MCF0499 plutôt que par un plantage.
//!
//! Règles d'exécution : `docs/grammaire/execution.md`.
//!
//! Tâches (`start` / `await`, DECISIONS D41) : chaque tâche a son propre fil système (sa pile),
//! mais **une seule s'exécute à la fois**. L'état partagé ([`World`]) est derrière un verrou que
//! seule la tâche « courante » détient ; elle ne le rend qu'aux points de suspension (`sleep`,
//! `await`, fin de tâche) après avoir désigné la suivante. L'ordre est donc déterministe :
//! file des tâches prêtes, puis minuteries dans l'ordre de leur échéance (horloge virtuelle).

mod value;

pub use value::{deep_copy, equal, StructValue, Value};

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::Scope;
use std::time::{Duration, Instant};

use mcf_diagnostics::{codes, Code, Diagnostic, SourceFile, Span};
use mcf_syntax::ast::*;
use value::{display, EnumInfo, StructInfo, TypeNames};

/// Profondeur d'appels maximale avant MCF0406 (récursion trop profonde).
pub const MAX_CALL_DEPTH: usize = 10_000;

/// Pile du fil d'exécution : assez grande pour `MAX_CALL_DEPTH` appels imbriqués.
const STACK_SIZE: usize = 512 * 1024 * 1024;

/// Un appel en cours au moment d'une erreur.
#[derive(Clone, Debug)]
pub struct TraceFrame {
    /// Nom qualifié : `Player.showState`, `main`, `on PlayerConnected`…
    pub function: String,
    /// Endroit de l'appel (fichier, position) ; `None` pour le point de départ.
    pub called_at: Option<(usize, Span)>,
}

#[derive(Clone, Debug)]
pub struct RuntimeError {
    /// Fichier où l'erreur se produit ; `None` pour une erreur de projet (pas de `main`).
    pub file: Option<usize>,
    pub diagnostic: Diagnostic,
    /// Appels en cours, du plus récent au plus ancien.
    pub trace: Vec<TraceFrame>,
}

/// Pourquoi l'exécution d'une tâche s'interrompt.
enum Stop {
    /// Erreur d'exécution : elle arrête tout le programme.
    Error(Box<RuntimeError>),
    /// La tâche a été annulée (`t.cancel()`) et a atteint un point de suspension.
    Cancelled,
    /// Une autre tâche a échoué : toutes les tâches s'arrêtent.
    Abort,
}

type Res<T> = Result<T, Stop>;

/// Exécute un projet vérifié : initialise les globales (fichiers dans l'ordre donné, puis
/// ordre de déclaration), appelle `func main(): Void`, puis attend la fin de toutes les tâches
/// lancées par `start`. Ce que `print` affiche va dans `out`. `files` (même ordre que
/// `modules`) sert à `meta.source` (« fichier:ligne » d'un `emit`).
pub fn run<W: Write + Send>(
    modules: &[&Module],
    files: &[SourceFile],
    out: &mut W,
) -> Result<(), Box<RuntimeError>> {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("mcf-run".into())
            .stack_size(STACK_SIZE)
            .spawn_scoped(s, || run_here(modules, files, out))
            .map_err(|e| failure(&format!("impossible de démarrer l'exécution : {e}")))?
            .join()
            .unwrap_or_else(|_| Err(failure("l'interpréteur s'est arrêté de façon inattendue")))
    })
}

fn failure(message: &str) -> Box<RuntimeError> {
    Box::new(RuntimeError {
        file: None,
        diagnostic: Diagnostic::new(&codes::INTERNAL_RUNTIME_ERROR, Span::new(0, 0), message),
        trace: Vec::new(),
    })
}

fn run_here<W: Write + Send>(
    modules: &[&Module],
    files: &[SourceFile],
    out: &mut W,
) -> Result<(), Box<RuntimeError>> {
    let program = Program::build(modules, files);
    let shared = Shared {
        world: Mutex::new(World {
            out,
            globals: vec![None; program.globals.len()],
            tasks: vec![TaskSlot::new()],
            current: 0,
            ready: VecDeque::new(),
            timers: BTreeMap::new(),
            seq: 0,
            clock: 0,
            failure: None,
            aborted: false,
            listeners: Vec::new(),
        }),
    };
    std::thread::scope(|scope| {
        let mut it = Interp {
            calls: Vec::new(),
            guard: Some(shared.lock()),
            p: &program,
            shared: &shared,
            scope,
            id: 0,
        };
        let result = it.start().map(|()| Value::Void);
        it.end_task(result);
    });
    // Toutes les tâches sont terminées : il ne reste que ce fil.
    let mut w = shared.lock();
    let _ = w.out.flush();
    match w.failure.take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ==================================================================== tâches

/// Ce qu'une tâche lancée par `start` doit exécuter ; les arguments sont déjà évalués.
enum Job {
    Func {
        id: usize,
        receiver: Option<Value>,
        args: Vec<Value>,
    },
    Builtin {
        builtin: Builtin,
        args: Vec<Value>,
        spans: Vec<Span>,
    },
    Native {
        receiver: Value,
        method: Ident,
        args: Vec<Value>,
        spans: Vec<Span>,
    },
    /// Gestionnaire `on` lancé par `start emit` : (événement, rang du gestionnaire).
    Handler {
        event: String,
        index: usize,
        args: Vec<Value>,
        meta: Option<Value>,
    },
}

#[derive(Debug)]
enum TaskState {
    /// En cours d'exécution, ou prête (dans la file `ready`).
    Active,
    /// `sleep` : clé de sa minuterie.
    Sleeping((u64, u64)),
    /// `await` d'une autre tâche.
    Waiting(usize),
    Done(Value),
    Cancelled,
}

struct TaskSlot {
    state: TaskState,
    /// `cancel()` demandé : la tâche s'arrête à son prochain point de suspension.
    cancel: bool,
    /// Tâches qui attendent celle-ci.
    waiters: Vec<usize>,
    /// Où la tâche attend (pour situer un blocage).
    waiting_at: Option<(usize, Span)>,
    /// Réveille le fil de cette tâche seulement (et non tous les fils) quand vient son tour.
    wake: Arc<Condvar>,
}

impl TaskSlot {
    fn new() -> Self {
        TaskSlot {
            state: TaskState::Active,
            cancel: false,
            waiters: Vec::new(),
            waiting_at: None,
            wake: Arc::new(Condvar::new()),
        }
    }

    fn finished(&self) -> bool {
        matches!(self.state, TaskState::Done(_) | TaskState::Cancelled)
    }
}

/// État partagé par toutes les tâches, accessible uniquement à la tâche courante.
struct World<'o, W> {
    out: &'o mut W,
    globals: Vec<Option<Value>>,
    /// Indice = numéro de tâche ; 0 = la tâche principale (`main`).
    tasks: Vec<TaskSlot>,
    /// Seule tâche autorisée à s'exécuter.
    current: usize,
    ready: VecDeque<usize>,
    /// (échéance virtuelle en ms, n° d'ordre) → (tâche, échéance réelle).
    timers: BTreeMap<(u64, u64), (usize, Instant)>,
    seq: u64,
    /// Horloge virtuelle (ms) : avance seulement quand une minuterie se déclenche, ce qui rend
    /// l'ordre des réveils indépendant de la vitesse de la machine.
    clock: u64,
    /// Première erreur d'exécution, rapportée à la fin.
    failure: Option<Box<RuntimeError>>,
    /// Une erreur s'est produite : toutes les tâches doivent s'arrêter.
    aborted: bool,
    /// Abonnements `on X => f`, dans l'ordre de création (D45).
    listeners: Vec<ListenerSlot>,
}

struct ListenerSlot {
    event: String,
    func: usize,
    active: bool,
}

impl<W> World<'_, W> {
    /// Donne la main à la tâche `next` et réveille son fil.
    fn hand_over(&mut self, next: usize) {
        self.current = next;
        self.tasks[next].wake.notify_one();
    }

    /// Arrêt général : toutes les tâches se réveillent pour s'arrêter.
    fn abort(&mut self) {
        self.aborted = true;
        for t in &self.tasks {
            t.wake.notify_one();
        }
    }
}

struct Shared<'o, W> {
    world: Mutex<World<'o, W>>,
}

impl<'o, W> Shared<'o, W> {
    fn lock(&self) -> MutexGuard<'_, World<'o, W>> {
        // Un verrou « empoisonné » vient d'une panique déjà en cours de traitement : on
        // poursuit l'arrêt.
        self.world.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Endort la tâche `id` jusqu'à son tour (ou jusqu'à l'arrêt général).
    fn wait_turn<'g>(
        &self,
        guard: MutexGuard<'g, World<'o, W>>,
        id: usize,
    ) -> MutexGuard<'g, World<'o, W>> {
        let wake = Arc::clone(&guard.tasks[id].wake);
        wake.wait_while(guard, |w| w.current != id && !w.aborted)
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Corps du fil d'une tâche lancée par `start`.
fn task_main<'scope, 'env, 'o, 'a, W: Write + Send>(
    p: &'env Program<'a>,
    shared: &'env Shared<'o, W>,
    scope: &'scope Scope<'scope, 'env>,
    id: usize,
    job: Job,
    called_at: (usize, Span),
) {
    let guard = shared.wait_turn(shared.lock(), id);
    let mut it = Interp {
        calls: Vec::new(),
        guard: Some(guard),
        p,
        shared,
        scope,
        id,
    };
    let result = if it.w().aborted {
        drop(job);
        Err(Stop::Abort)
    } else if it.w().tasks[id].cancel {
        drop(job);
        Err(Stop::Cancelled)
    } else {
        it.run_job(job, called_at)
    };
    it.end_task(result);
}

// ==================================================================== programme

#[derive(Clone, Copy, Debug, PartialEq)]
enum Builtin {
    Print,
    Sleep,
    Duplicate,
}

/// Ce que désigne un nom déclaré (miroir des symboles de `mcf_semantic`).
#[derive(Clone, Copy, Debug)]
enum Entity {
    Global(usize),
    Func(usize),
    Scope(usize),
    Struct(usize),
    Enum(usize),
    Builtin(Builtin),
}

struct Table {
    parent: Option<usize>,
    names: HashMap<String, Entity>,
}

struct FuncInfo<'a> {
    name: String,
    decl: &'a FuncDecl,
    table: usize,
    file: usize,
}

struct GlobalInfo<'a> {
    name: String,
    decl: &'a VarDecl,
    table: usize,
    file: usize,
}

struct Handler<'a> {
    decl: &'a OnHandler,
    table: usize,
    file: usize,
}

/// Événement déclaré : de quoi calculer ses métadonnées à chaque `emit`.
struct EventInfo<'a> {
    decl: &'a EventDecl,
    table: usize,
    file: usize,
    /// Structure `X.meta` dans `types.structs`.
    meta: usize,
}

struct Program<'a> {
    events: HashMap<String, EventInfo<'a>>,
    files: &'a [SourceFile],
    tables: Vec<Table>,
    roots: Vec<usize>,
    /// Par fichier : nom importé → (table d'origine, nom).
    imports: Vec<HashMap<String, (usize, String)>>,
    funcs: Vec<FuncInfo<'a>>,
    globals: Vec<GlobalInfo<'a>>,
    /// Ordre d'initialisation : fichiers dans l'ordre, déclarations dans l'ordre.
    types: TypeNames,
    handlers: HashMap<String, Vec<Handler<'a>>>,
    /// Méthodes des structures : (structure, nom) → fonction.
    methods: HashMap<(usize, String), usize>,
}

/// Nom de la locale qui porte le receveur d'une méthode (`self` est un mot-clé).
const SELF_LOCAL: &str = "self";

/// Nom de la locale qui porte les métadonnées dans un gestionnaire `on` (D46).
const META_LOCAL: &str = "meta";

fn path_text(path: &Path) -> String {
    path.segments
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

impl<'a> Program<'a> {
    fn build(modules: &[&'a Module], files: &'a [SourceFile]) -> Self {
        let mut p = Program {
            events: HashMap::new(),
            files,
            tables: Vec::new(),
            roots: Vec::new(),
            imports: vec![HashMap::new(); modules.len()],
            funcs: Vec::new(),
            globals: Vec::new(),
            types: TypeNames {
                structs: Vec::new(),
                enums: Vec::new(),
            },
            handlers: HashMap::new(),
            methods: HashMap::new(),
        };
        for (file, module) in modules.iter().enumerate() {
            let root = p.new_table(None);
            p.roots.push(root);
            p.declare(&module.items, root, file, "");
        }
        for (file, module) in modules.iter().enumerate() {
            for item in &module.items {
                if let Item::Import(imp) = item {
                    if let Some(found) = p.find_exported(&imp.path.segments, file) {
                        p.imports[file].insert(imp.local_name().name.clone(), found);
                    }
                }
            }
        }
        p
    }

    fn new_table(&mut self, parent: Option<usize>) -> usize {
        self.tables.push(Table {
            parent,
            names: HashMap::new(),
        });
        self.tables.len() - 1
    }

    fn declare(&mut self, items: &'a [Item], table: usize, file: usize, prefix: &str) {
        for item in items {
            let (name, entity) = match item {
                Item::Global(v) => {
                    self.globals.push(GlobalInfo {
                        name: format!("{prefix}{}", v.name.name),
                        decl: v,
                        table,
                        file,
                    });
                    (&v.name.name, Entity::Global(self.globals.len() - 1))
                }
                Item::Func(f) => {
                    self.funcs.push(FuncInfo {
                        name: format!("{prefix}{}", f.name.name),
                        decl: f,
                        table,
                        file,
                    });
                    (&f.name.name, Entity::Func(self.funcs.len() - 1))
                }
                Item::Scope(s) => {
                    let child = self.new_table(Some(table));
                    self.declare(&s.items, child, file, &format!("{prefix}{}.", s.name.name));
                    (&s.name.name, Entity::Scope(child))
                }
                Item::Struct(s) => {
                    let id = self.types.structs.len();
                    self.types.structs.push(StructInfo {
                        name: s.name.name.clone(),
                        fields: s.fields.iter().map(|f| f.name.name.clone()).collect(),
                    });
                    for m in &s.methods {
                        self.funcs.push(FuncInfo {
                            name: format!("{prefix}{}.{}", s.name.name, m.name.name),
                            decl: m,
                            table,
                            file,
                        });
                        self.methods
                            .insert((id, m.name.name.clone()), self.funcs.len() - 1);
                    }
                    (&s.name.name, Entity::Struct(id))
                }
                Item::Enum(e) => {
                    self.types.enums.push(EnumInfo {
                        name: e.name.name.clone(),
                        variants: e.variants.iter().map(|v| v.name.clone()).collect(),
                    });
                    (&e.name.name, Entity::Enum(self.types.enums.len() - 1))
                }
                Item::On(h) => {
                    self.handlers
                        .entry(path_text(&h.event))
                        .or_default()
                        .push(Handler {
                            decl: h,
                            table,
                            file,
                        });
                    continue;
                }
                Item::Event(e) => {
                    let name = path_text(&e.name);
                    let meta = self.types.structs.len();
                    self.types.structs.push(StructInfo {
                        name: format!("{name}.meta"),
                        fields: e
                            .meta
                            .iter()
                            .map(|m| m.name.name.clone())
                            .chain(std::iter::once("source".to_string()))
                            .collect(),
                    });
                    self.events.entry(name).or_insert(EventInfo {
                        decl: e,
                        table,
                        file,
                        meta,
                    });
                    continue;
                }
                Item::Import(_) => continue,
            };
            self.tables[table].names.insert(name.clone(), entity);
        }
    }

    /// Même règle que `mcf_semantic` : l'élément est cherché depuis la racine des autres fichiers.
    fn find_exported(&self, segments: &[Ident], from: usize) -> Option<(usize, String)> {
        for (file, &root) in self.roots.iter().enumerate() {
            if file == from {
                continue;
            }
            let mut table = root;
            for (i, seg) in segments.iter().enumerate() {
                match self.tables[table].names.get(&seg.name) {
                    Some(Entity::Scope(t)) if i + 1 < segments.len() => table = *t,
                    Some(_) if i + 1 == segments.len() => return Some((table, seg.name.clone())),
                    _ => break,
                }
            }
        }
        None
    }

    fn entry_point(&self) -> Option<usize> {
        self.roots
            .iter()
            .find_map(|&root| match self.tables[root].names.get("main") {
                Some(Entity::Func(id)) => Some(*id),
                _ => None,
            })
    }
}

// ==================================================================== interpréteur

/// Une variable locale : `None` tant qu'elle n'a pas reçu de valeur.
type Locals = HashMap<String, Option<Value>>;

struct CallFrame {
    name: String,
    file: usize,
    table: usize,
    called_at: Option<(usize, Span)>,
    /// Blocs ouverts, du plus externe au plus interne.
    blocks: Vec<Locals>,
}

enum Flow {
    Next,
    Break,
    Continue,
    Return(Value),
}

/// Type testé par `is`, résolu à l'exécution.
enum Kind {
    Int,
    Float,
    Bool,
    String,
    Array,
    Map,
    Task,
    Listener,
    Struct(usize),
    Enum(usize),
    /// Type sans valeur possible aujourd'hui (`Char`, `Byte`, `Bytes`, `DateTime`).
    NoValue,
}

/// Une tâche en cours d'exécution.
///
/// `calls` est déclaré avant `guard` : la pile est libérée verrou détenu, même si le fil panique.
struct Interp<'scope, 'env, 'o, 'a, W: Write + Send> {
    calls: Vec<CallFrame>,
    /// Verrou de l'état partagé ; `None` seulement pendant une suspension ou après la fin.
    guard: Option<MutexGuard<'env, World<'o, W>>>,
    p: &'env Program<'a>,
    shared: &'env Shared<'o, W>,
    scope: &'scope Scope<'scope, 'env>,
    /// Numéro de cette tâche.
    id: usize,
}

impl<W: Write + Send> Drop for Interp<'_, '_, '_, '_, W> {
    fn drop(&mut self) {
        // Panique (bug de l'interpréteur) : arrêter les autres tâches au lieu de les laisser
        // attendre un tour qui ne viendrait jamais.
        if std::thread::panicking() {
            if let Some(w) = self.guard.as_mut() {
                w.abort();
            }
        }
    }
}

impl<'scope, 'env, 'o, 'a, W: Write + Send> Interp<'scope, 'env, 'o, 'a, W> {
    fn w(&self) -> &World<'o, W> {
        self.guard
            .as_ref()
            .expect("la tâche courante détient le verrou")
    }

    fn wm(&mut self) -> &mut World<'o, W> {
        self.guard
            .as_mut()
            .expect("la tâche courante détient le verrou")
    }

    fn start(&mut self) -> Res<()> {
        let p = self.p;
        let Some(main) = p.entry_point() else {
            return Err(Stop::Error(Box::new(RuntimeError {
                file: None,
                diagnostic: Diagnostic::new(
                    &codes::MISSING_ENTRY_POINT,
                    Span::new(0, 0),
                    "le projet n'a pas de point d'entrée `main`",
                )
                .suggestion("ajouter `func main(): Void { ... }` à la racine d'un fichier"),
                trace: Vec::new(),
            })));
        };
        for (slot, g) in p.globals.iter().enumerate() {
            self.calls.push(CallFrame {
                name: format!("initialisation de `{}`", g.name),
                file: g.file,
                table: g.table,
                called_at: None,
                blocks: Vec::new(),
            });
            let value = match &g.decl.init {
                Some(init) => Some(self.eval(init)?),
                None => default_value(&g.decl.ty),
            };
            self.wm().globals[slot] = value;
            self.calls.pop();
        }
        self.call_func(main, None, Vec::new(), None)?;
        // Le programme se termine quand toutes les tâches lancées sont terminées.
        let mut t = 1;
        while t < self.w().tasks.len() {
            self.wait_for(t, None)?;
            t += 1;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- ordonnancement

    /// Désigne la prochaine tâche à exécuter : d'abord les tâches prêtes, puis la minuterie la
    /// plus proche (on attend alors réellement son échéance). `None` : plus rien ne peut avancer.
    fn pick_next(&mut self) -> Option<usize> {
        let w = self.wm();
        if let Some(next) = w.ready.pop_front() {
            return Some(next);
        }
        let (&key, &(next, deadline)) = w.timers.iter().next()?;
        w.timers.remove(&key);
        w.clock = w.clock.max(key.0);
        w.tasks[next].state = TaskState::Active;
        let _ = w.out.flush();
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        Some(next)
    }

    /// Erreur « les tâches s'attendent », située sur une attente quand on la connaît.
    fn deadlock(at: Option<(usize, Span)>, message: &str) -> Box<RuntimeError> {
        let (file, span) = at.map_or((None, Span::new(0, 0)), |(f, s)| (Some(f), s));
        Box::new(RuntimeError {
            file,
            diagnostic: Diagnostic::new(&codes::DEADLOCK, span, message).suggestion(
                "vérifier qu'aucune tâche n'attend, même indirectement, une tâche qui l'attend",
            ),
            trace: Vec::new(),
        })
    }

    /// Point de suspension : l'état de cette tâche a déjà été mis à jour (`Sleeping`,
    /// `Waiting`). Passe la main et revient quand c'est de nouveau son tour.
    fn suspend(&mut self, at: Option<(usize, Span)>) -> Res<()> {
        let Some(next) = self.pick_next() else {
            return Err(Stop::Error(Self::deadlock(
                at,
                "toutes les tâches s'attendent : le programme ne peut plus avancer",
            )));
        };
        if next != self.id {
            self.wm().hand_over(next);
            let guard = self.guard.take().expect("verrou détenu");
            self.guard = Some(self.shared.wait_turn(guard, self.id));
        }
        let id = self.id;
        let w = self.w();
        if w.aborted {
            return Err(Stop::Abort);
        }
        if w.tasks[id].cancel {
            return Err(Stop::Cancelled);
        }
        Ok(())
    }

    /// `sleep(ms)` : laisse tourner les autres tâches pendant au moins `ms` millisecondes.
    fn sleep(&mut self, ms: u64) -> Res<()> {
        let id = self.id;
        let w = self.wm();
        let key = (w.clock.saturating_add(ms), w.seq);
        w.seq += 1;
        w.timers
            .insert(key, (id, Instant::now() + Duration::from_millis(ms)));
        w.tasks[id].state = TaskState::Sleeping(key);
        self.suspend(None)
    }

    /// Attend que la tâche `t` soit terminée (ou annulée).
    fn wait_for(&mut self, t: usize, at: Option<(usize, Span)>) -> Res<()> {
        if t == self.id {
            return Err(Stop::Error(Self::deadlock(
                at,
                "une tâche ne peut pas s'attendre elle-même",
            )));
        }
        while !self.w().tasks[t].finished() {
            let id = self.id;
            let w = self.wm();
            w.tasks[t].waiters.push(id);
            w.tasks[id].state = TaskState::Waiting(t);
            w.tasks[id].waiting_at = at;
            self.suspend(at)?;
        }
        Ok(())
    }

    /// `await t` : le résultat de la tâche, ou MCF0410 si elle a été annulée.
    fn await_task(&mut self, t: usize, span: Span) -> Res<Value> {
        let at = Some((self.frame().file, span));
        self.wait_for(t, at)?;
        match &self.w().tasks[t].state {
            TaskState::Done(v) => Ok(v.clone()),
            _ => Err(self.error_with(
                Diagnostic::new(
                    &codes::TASK_CANCELLED,
                    span,
                    "cette tâche a été annulée : elle n'a pas de résultat",
                )
                .suggestion("vérifier avec `isDone()` et ne pas attendre une tâche annulée"),
            )),
        }
    }

    /// `start` : crée la tâche (prête, après celles déjà en file) et son fil.
    fn spawn(&mut self, job: Job, called_at: (usize, Span)) -> Res<Value> {
        let id = self.w().tasks.len();
        let w = self.wm();
        w.tasks.push(TaskSlot::new());
        w.ready.push_back(id);
        let (p, shared, scope) = (self.p, self.shared, self.scope);
        let spawned = std::thread::Builder::new()
            .name(format!("mcf-task-{id}"))
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, move || {
                task_main(p, shared, scope, id, job, called_at)
            });
        if let Err(e) = spawned {
            let w = self.wm();
            w.ready.retain(|&t| t != id);
            w.tasks[id].state = TaskState::Cancelled;
            return Err(self.error(
                &codes::TASK_START_FAILED,
                called_at.1,
                format!("impossible de démarrer la tâche : {e}"),
            ));
        }
        Ok(Value::Task(id))
    }

    fn run_job(&mut self, job: Job, called_at: (usize, Span)) -> Res<Value> {
        if let Job::Handler {
            event,
            index,
            args,
            meta,
        } = job
        {
            self.run_handler(&event, index, args, meta, called_at)?;
            return Ok(Value::Void);
        }
        if matches!(job, Job::Func { .. }) {
            return self.invoke(job, called_at);
        }
        // Fonction ou méthode native : un cadre d'appel situe les erreurs éventuelles.
        self.calls.push(CallFrame {
            name: "tâche".into(),
            file: called_at.0,
            table: self.p.roots[called_at.0],
            called_at: Some(called_at),
            blocks: Vec::new(),
        });
        let v = self.invoke(job, called_at)?;
        self.calls.pop();
        Ok(v)
    }

    /// Fin de tâche : enregistre le résultat, réveille ceux qui l'attendent, passe la main.
    fn end_task(mut self, result: Res<Value>) {
        self.calls.clear();
        let id = self.id;
        let w = self.wm();
        w.tasks[id].state = match result {
            Ok(v) => TaskState::Done(v),
            Err(Stop::Error(e)) => {
                w.failure.get_or_insert(e);
                w.abort();
                TaskState::Cancelled
            }
            Err(Stop::Cancelled | Stop::Abort) => TaskState::Cancelled,
        };
        for waiter in std::mem::take(&mut w.tasks[id].waiters) {
            w.tasks[waiter].state = TaskState::Active;
            w.ready.push_back(waiter);
        }
        if !w.aborted {
            match self.pick_next() {
                Some(next) => self.wm().hand_over(next),
                None => {
                    let w = self.wm();
                    if let Some(stuck) = w.tasks.iter().find(|t| !t.finished()) {
                        let e = Self::deadlock(
                            stuck.waiting_at,
                            "toutes les tâches s'attendent : le programme ne peut plus avancer",
                        );
                        w.failure.get_or_insert(e);
                        w.abort();
                    }
                }
            }
        }
        self.guard = None;
    }

    /// `t.cancel()` : sans effet sur une tâche terminée ; sinon la tâche s'arrête à son
    /// prochain point de suspension (tout de suite si elle dort ou attend).
    fn cancel(&mut self, t: usize) {
        let w = self.wm();
        if w.tasks[t].finished() {
            return;
        }
        w.tasks[t].cancel = true;
        match std::mem::replace(&mut w.tasks[t].state, TaskState::Active) {
            TaskState::Sleeping(key) => {
                w.timers.remove(&key);
                w.ready.push_back(t);
            }
            TaskState::Waiting(target) => {
                w.tasks[target].waiters.retain(|&x| x != t);
                w.ready.push_back(t);
            }
            other => w.tasks[t].state = other,
        }
    }

    // ---------------------------------------------------------------- erreurs

    fn frame(&self) -> &CallFrame {
        self.calls.last().expect("un appel est toujours en cours")
    }

    fn error(&self, code: &'static Code, span: Span, message: impl Into<String>) -> Stop {
        self.error_with(Diagnostic::new(code, span, message))
    }

    fn error_with(&self, diagnostic: Diagnostic) -> Stop {
        Stop::Error(Box::new(RuntimeError {
            file: self.calls.last().map(|f| f.file),
            diagnostic,
            trace: self
                .calls
                .iter()
                .rev()
                .map(|f| TraceFrame {
                    function: f.name.clone(),
                    called_at: f.called_at,
                })
                .collect(),
        }))
    }

    /// Situation que la vérification aurait dû empêcher.
    fn internal(&self, span: Span, what: &str) -> Stop {
        self.error_with(
            Diagnostic::new(
                &codes::INTERNAL_RUNTIME_ERROR,
                span,
                format!("erreur interne de l'interpréteur : {what}"),
            )
            .suggestion(
                "ce programme a passé la vérification : merci de signaler ce cas à MCF Studio",
            ),
        )
    }

    fn unset(&self, name: &str, span: Span) -> Stop {
        self.error_with(
            Diagnostic::new(
                &codes::UNINITIALIZED_VARIABLE,
                span,
                format!("`{name}` est lue avant d'avoir reçu une valeur"),
            )
            .suggestion(format!("donner une valeur à `{name}` dès sa déclaration")),
        )
    }

    // ---------------------------------------------------------------- noms

    fn find_local(&self, name: &str) -> Option<&Option<Value>> {
        self.frame().blocks.iter().rev().find_map(|b| b.get(name))
    }

    fn find_local_mut(&mut self, name: &str) -> Option<&mut Option<Value>> {
        let frame = self.calls.last_mut()?;
        frame.blocks.iter_mut().rev().find_map(|b| b.get_mut(name))
    }

    /// Blocs locaux exclus : scopes englobants → racine du fichier → imports → natifs.
    fn lookup(&self, name: &str) -> Option<Entity> {
        let p = self.p;
        let frame = self.frame();
        let mut t = Some(frame.table);
        while let Some(id) = t {
            if let Some(e) = p.tables[id].names.get(name) {
                return Some(*e);
            }
            t = p.tables[id].parent;
        }
        if let Some((t, n)) = p.imports[frame.file].get(name) {
            return p.tables[*t].names.get(n).copied();
        }
        match name {
            "print" => Some(Entity::Builtin(Builtin::Print)),
            "sleep" => Some(Entity::Builtin(Builtin::Sleep)),
            "duplicate" => Some(Entity::Builtin(Builtin::Duplicate)),
            _ => None,
        }
    }

    fn resolve_path(&self, path: &Path) -> Option<Entity> {
        let mut entity = self.lookup(&path.segments[0].name)?;
        for seg in &path.segments[1..] {
            let Entity::Scope(t) = entity else {
                return None;
            };
            entity = *self.p.tables[t].names.get(&seg.name)?;
        }
        Some(entity)
    }

    /// Ce que désigne `expr` s'il s'agit d'un nom déclaré (pas d'une valeur locale).
    /// Ne s'évalue pas : un chemin `A.B.c` n'a pas d'effet de bord.
    fn entity_of(&self, expr: &Expr) -> Option<Entity> {
        match &expr.kind {
            ExprKind::Name { ident } => {
                if self.find_local(&ident.name).is_some() {
                    None
                } else {
                    self.lookup(&ident.name)
                }
            }
            ExprKind::Member { object, field } => match self.entity_of(object)? {
                Entity::Scope(t) => self.p.tables[t].names.get(&field.name).copied(),
                _ => None,
            },
            _ => None,
        }
    }

    fn read_global(&self, slot: usize, span: Span) -> Res<Value> {
        self.w().globals[slot]
            .clone()
            .ok_or_else(|| self.unset(&self.p.globals[slot].name, span))
    }

    // ---------------------------------------------------------------- appels

    /// Appelle une fonction, ou une méthode quand `receiver` est donné (il devient `self`).
    fn call_func(
        &mut self,
        id: usize,
        receiver: Option<Value>,
        args: Vec<Value>,
        called_at: Option<(usize, Span)>,
    ) -> Res<Value> {
        let p = self.p;
        let f = &p.funcs[id];
        if self.calls.len() >= MAX_CALL_DEPTH {
            let span = called_at.map_or(f.decl.name.span, |(_, s)| s);
            return Err(self.error_with(
                Diagnostic::new(
                    &codes::STACK_OVERFLOW,
                    span,
                    format!("récursion trop profonde : plus de {MAX_CALL_DEPTH} appels imbriqués"),
                )
                .suggestion("vérifier la condition d'arrêt de la fonction récursive"),
            ));
        }
        let mut locals: Locals = f
            .decl
            .params
            .iter()
            .zip(args)
            .map(|(param, v)| (param.name.name.clone(), Some(v)))
            .collect();
        if let Some(recv) = receiver {
            locals.insert(SELF_LOCAL.to_string(), Some(recv));
        }
        self.calls.push(CallFrame {
            name: f.name.clone(),
            file: f.file,
            table: f.table,
            called_at,
            blocks: vec![locals],
        });
        let flow = self.exec_block(&f.decl.body)?;
        self.calls.pop();
        Ok(match flow {
            Flow::Return(v) => v,
            _ => Value::Void,
        })
    }

    /// Évalue ce qu'un appel désigne et ses arguments (le receveur d'une méthode d'abord),
    /// sans l'exécuter : partagé par l'appel direct et par `start`.
    fn prepare_call(&mut self, callee: &Expr, args: &[Expr]) -> Res<Job> {
        let target = self.entity_of(callee);
        let spans: Vec<Span> = args.iter().map(|a| a.span).collect();
        if let (None, ExprKind::Member { object, field }) = (target, &callee.kind) {
            let receiver = self.eval(object)?;
            let args = args.iter().map(|a| self.eval(a)).collect::<Res<Vec<_>>>()?;
            if let Value::Struct(s) = &receiver {
                let key = (s.borrow().id, field.name.clone());
                let Some(&id) = self.p.methods.get(&key) else {
                    return Err(self.internal(field.span, "méthode de structure inconnue"));
                };
                return Ok(Job::Func {
                    id,
                    receiver: Some(receiver),
                    args,
                });
            }
            return Ok(Job::Native {
                receiver,
                method: field.clone(),
                args,
                spans,
            });
        }
        let args = args.iter().map(|a| self.eval(a)).collect::<Res<Vec<_>>>()?;
        match target {
            Some(Entity::Func(id)) => Ok(Job::Func {
                id,
                receiver: None,
                args,
            }),
            Some(Entity::Builtin(builtin)) => Ok(Job::Builtin {
                builtin,
                args,
                spans,
            }),
            _ => Err(self.internal(
                callee.span,
                "appel de quelque chose qui n'est pas une fonction",
            )),
        }
    }

    fn invoke(&mut self, job: Job, called_at: (usize, Span)) -> Res<Value> {
        match job {
            Job::Func { id, receiver, args } => self.call_func(id, receiver, args, Some(called_at)),
            Job::Builtin {
                builtin,
                args,
                spans,
            } => self.call_builtin(builtin, args, &spans),
            Job::Native {
                receiver,
                method,
                args,
                spans,
            } => self.call_method(receiver, &method, args, &spans),
            Job::Handler {
                event,
                index,
                args,
                meta,
            } => {
                self.run_handler(&event, index, args, meta, called_at)?;
                Ok(Value::Void)
            }
        }
    }

    /// `emit` : les gestionnaires `on` s'exécutent tout de suite, dans l'ordre du projet
    /// (DECISIONS D32).
    ///
    /// `start emit` (`background`) : chaque gestionnaire devient une tâche, lancée dans le
    /// même ordre ; l'émetteur continue aussitôt (D44).
    ///
    /// Les gestionnaires `on` fixes passent d'abord, puis les abonnements `on X => f` actifs,
    /// dans l'ordre où ils ont été créés (D45). Un abonnement créé pendant l'`emit` attend le
    /// suivant ; un abonnement arrêté pendant l'`emit` n'est plus appelé.
    fn emit(
        &mut self,
        event: &Path,
        args: Vec<Value>,
        meta: &[FieldInit],
        background: bool,
        span: Span,
    ) -> Res<()> {
        let p = self.p;
        let name = path_text(event);
        let count = p.handlers.get(&name).map_or(0, Vec::len);
        let called_at = (self.frame().file, span);
        let meta = self.build_meta(&name, &args, meta, called_at)?;
        for index in 0..count {
            if background {
                let job = Job::Handler {
                    event: name.clone(),
                    index,
                    args: args.clone(),
                    meta: meta.clone(),
                };
                self.spawn(job, called_at)?;
            } else {
                self.run_handler(&name, index, args.clone(), meta.clone(), called_at)?;
            }
        }
        let subscribed: Vec<usize> = (0..self.w().listeners.len())
            .filter(|&l| self.w().listeners[l].event == name)
            .collect();
        for l in subscribed {
            let slot = &self.w().listeners[l];
            if !slot.active {
                continue;
            }
            let func = slot.func;
            if background {
                let job = Job::Func {
                    id: func,
                    receiver: None,
                    args: args.clone(),
                };
                self.spawn(job, called_at)?;
            } else {
                self.call_func(func, None, args.clone(), Some(called_at))?;
            }
        }
        Ok(())
    }

    /// Métadonnées d'un `emit` (D46) : les valeurs données (calculées chez l'émetteur, dans
    /// l'ordre écrit), puis les valeurs par défaut des autres (calculées dans le contexte de
    /// la déclaration, données de l'événement lisibles), puis `source`.
    fn build_meta(
        &mut self,
        event: &str,
        args: &[Value],
        inits: &[FieldInit],
        called_at: (usize, Span),
    ) -> Res<Option<Value>> {
        let p = self.p;
        let Some(info) = p.events.get(event) else {
            return Ok(None);
        };
        let mut given = Vec::new();
        for f in inits {
            given.push((f.name.name.as_str(), self.eval(&f.value)?));
        }
        let locals: Locals = info
            .decl
            .params
            .iter()
            .zip(args.iter().cloned())
            .map(|(param, v)| (param.name.name.clone(), Some(v)))
            .collect();
        self.calls.push(CallFrame {
            name: format!("métadonnées de {event}"),
            file: info.file,
            table: info.table,
            called_at: Some(called_at),
            blocks: vec![locals],
        });
        let mut fields = Vec::new();
        for m in &info.decl.meta {
            match given.iter().position(|(n, _)| *n == m.name.name) {
                Some(i) => fields.push(given.swap_remove(i).1),
                None => fields.push(self.eval(&m.default)?),
            }
        }
        self.calls.pop();
        let source = match p.files.get(called_at.0) {
            Some(f) => format!("{}:{}", f.name, f.location(called_at.1.start).line),
            None => "?".to_string(),
        };
        fields.push(Value::Str(source.into()));
        Ok(Some(Value::Struct(std::sync::Arc::new(
            value::ValueCell::new(StructValue {
                id: info.meta,
                fields,
            }),
        ))))
    }

    /// Exécute le gestionnaire `on` n° `index` de l'événement `event`.
    fn run_handler(
        &mut self,
        event: &str,
        index: usize,
        args: Vec<Value>,
        meta: Option<Value>,
        called_at: (usize, Span),
    ) -> Res<()> {
        let p = self.p;
        let Some(h) = p.handlers.get(event).and_then(|hs| hs.get(index)) else {
            return Err(self.internal(called_at.1, "gestionnaire d'événement inconnu"));
        };
        if self.calls.len() >= MAX_CALL_DEPTH {
            return Err(self.error(
                &codes::STACK_OVERFLOW,
                called_at.1,
                format!("trop d'événements imbriqués : plus de {MAX_CALL_DEPTH} appels"),
            ));
        }
        let mut locals: Locals = h
            .decl
            .bindings
            .iter()
            .zip(args)
            .map(|(b, v)| (b.name.clone(), Some(v)))
            .collect();
        if let Some(m) = meta {
            locals.insert(META_LOCAL.to_string(), Some(m));
        }
        self.calls.push(CallFrame {
            name: format!("on {event}"),
            file: h.file,
            table: h.table,
            called_at: Some(called_at),
            blocks: vec![locals],
        });
        self.exec_block(&h.decl.body)?;
        self.calls.pop();
        Ok(())
    }

    fn call_builtin(&mut self, b: Builtin, args: Vec<Value>, arg_spans: &[Span]) -> Res<Value> {
        let [arg] = args.as_slice() else {
            return Err(self.internal(
                arg_spans.first().copied().unwrap_or(Span::new(0, 0)),
                "nombre d'arguments",
            ));
        };
        match b {
            Builtin::Duplicate => return Ok(deep_copy(arg)),
            Builtin::Print => {
                let text = display(arg, &self.p.types);
                if writeln!(self.wm().out, "{text}").is_err() {
                    return Err(self.error(
                        &codes::OUTPUT_ERROR,
                        arg_spans[0],
                        "impossible d'écrire sur la sortie",
                    ));
                }
            }
            Builtin::Sleep => {
                let Value::Int(ms) = arg else {
                    return Err(self.internal(arg_spans[0], "`sleep` sans entier"));
                };
                let Ok(ms) = u64::try_from(*ms) else {
                    return Err(self.error_with(
                        Diagnostic::new(
                            &codes::INVALID_ARGUMENT,
                            arg_spans[0],
                            "`sleep` attend une durée positive ou nulle (millisecondes)",
                        )
                        .received(ms.to_string()),
                    ));
                };
                self.sleep(ms)?;
            }
        }
        Ok(Value::Void)
    }

    /// Méthodes natives (DECISIONS D38). Les signatures ont été vérifiées par `mcf_semantic`.
    fn call_method(
        &mut self,
        recv: Value,
        method: &Ident,
        mut args: Vec<Value>,
        arg_spans: &[Span],
    ) -> Res<Value> {
        let arg = args.pop();
        let length = |n: usize| Value::Int(i64::try_from(n).unwrap_or(i64::MAX));
        match (&recv, method.name.as_str(), arg) {
            (Value::Str(s), "length", None) => Ok(length(s.chars().count())),
            (Value::Task(t), "cancel", None) => {
                self.cancel(*t);
                Ok(Value::Void)
            }
            (Value::Task(t), "isDone", None) => Ok(Value::Bool(self.w().tasks[*t].finished())),
            (Value::Listener(l), "stop", None) => {
                self.wm().listeners[*l].active = false;
                Ok(Value::Void)
            }
            (Value::Listener(l), "isActive", None) => {
                Ok(Value::Bool(self.w().listeners[*l].active))
            }
            (Value::Array(a), "length", None) => Ok(length(a.borrow().len())),
            (Value::Map(m), "length", None) => Ok(length(m.borrow().len())),
            (Value::Array(a), "push", Some(v)) => {
                a.borrow_mut().push(v);
                Ok(Value::Void)
            }
            (Value::Array(a), "pop", None) => {
                let last = a.borrow_mut().pop();
                last.ok_or_else(|| {
                    self.error_with(
                        Diagnostic::new(
                            &codes::EMPTY_COLLECTION,
                            method.span,
                            "`pop` sur un tableau vide",
                        )
                        .suggestion("vérifier d'abord `tableau.length() > 0`"),
                    )
                })
            }
            (Value::Array(a), "removeAt", Some(i)) => {
                let index = self.array_index(&a.borrow(), &i, arg_spans[0])?;
                let removed = a.borrow_mut().remove(index);
                Ok(removed)
            }
            (Value::Array(a), "contains", Some(v)) => {
                let items = a.borrow().clone();
                Ok(Value::Bool(items.iter().any(|item| equal(item, &v))))
            }
            (Value::Map(m), "has", Some(k)) => {
                let entries = m.borrow().clone();
                Ok(Value::Bool(map_position(&entries, &k).is_some()))
            }
            (Value::Map(m), "remove", Some(k)) => {
                let entries = m.borrow().clone();
                let found = map_position(&entries, &k);
                if let Some(i) = found {
                    m.borrow_mut().remove(i);
                }
                Ok(Value::Bool(found.is_some()))
            }
            (Value::Map(m), "keys", None) => Ok(Value::new_array(
                m.borrow().iter().map(|(k, _)| k.clone()).collect(),
            )),
            (Value::Map(m), "values", None) => Ok(Value::new_array(
                m.borrow().iter().map(|(_, v)| v.clone()).collect(),
            )),
            _ => Err(self.internal(method.span, "méthode native inconnue")),
        }
    }

    // ---------------------------------------------------------------- instructions

    fn exec_block(&mut self, block: &Block) -> Res<Flow> {
        self.calls
            .last_mut()
            .expect("un appel est en cours")
            .blocks
            .push(Locals::new());
        let mut flow = Flow::Next;
        for stmt in &block.stmts {
            flow = self.exec_stmt(stmt)?;
            if !matches!(flow, Flow::Next) {
                break;
            }
        }
        self.calls
            .last_mut()
            .expect("un appel est en cours")
            .blocks
            .pop();
        Ok(flow)
    }

    fn declare_local(&mut self, name: &str, value: Option<Value>) {
        if let Some(block) = self.calls.last_mut().and_then(|f| f.blocks.last_mut()) {
            block.insert(name.to_string(), value);
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Res<Flow> {
        match stmt {
            Stmt::Local(v) => {
                // La valeur est calculée avant la déclaration (comme à la vérification).
                let value = match &v.init {
                    Some(init) => Some(self.eval(init)?),
                    None => default_value(&v.ty),
                };
                self.declare_local(&v.name.name, value);
            }
            Stmt::Assign { target, value, .. } => self.assign(target, value)?,
            Stmt::If(i) => return self.exec_if(i),
            Stmt::While { cond, body, .. } => {
                while self.eval_bool(cond)? {
                    match self.exec_block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                }
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                self.calls
                    .last_mut()
                    .expect("appel en cours")
                    .blocks
                    .push(Locals::new());
                let result = self.exec_for(init, cond, step, body);
                self.calls.last_mut().expect("appel en cours").blocks.pop();
                return result;
            }
            Stmt::Foreach {
                binding,
                iter,
                body,
                ..
            } => {
                // On parcourt le contenu tel qu'il est au début de la boucle.
                let items: Vec<Value> = match self.eval(iter)? {
                    Value::Array(a) => a.borrow().clone(),
                    Value::Map(m) => m.borrow().iter().map(|(k, _)| k.clone()).collect(),
                    _ => {
                        return Err(
                            self.internal(iter.span, "`foreach` sur une valeur non parcourable")
                        )
                    }
                };
                for item in items {
                    self.calls
                        .last_mut()
                        .expect("appel en cours")
                        .blocks
                        .push(Locals::new());
                    self.declare_local(&binding.name, Some(item));
                    let flow = self.exec_block(body);
                    self.calls.last_mut().expect("appel en cours").blocks.pop();
                    match flow? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                }
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval(e)?,
                    None => Value::Void,
                };
                return Ok(Flow::Return(v));
            }
            Stmt::Break { .. } => return Ok(Flow::Break),
            Stmt::Continue { .. } => return Ok(Flow::Continue),
            Stmt::Emit {
                event,
                args,
                meta,
                background,
                span,
            } => {
                let values = args.iter().map(|a| self.eval(a)).collect::<Res<Vec<_>>>()?;
                self.emit(event, values, meta, *background, *span)?;
            }
            Stmt::Expr { expr, .. } => {
                self.eval(expr)?;
            }
        }
        Ok(Flow::Next)
    }

    fn exec_for(&mut self, init: &VarDecl, cond: &Expr, step: &Stmt, body: &Block) -> Res<Flow> {
        self.exec_stmt(&Stmt::Local(init.clone()))?;
        while self.eval_bool(cond)? {
            match self.exec_block(body)? {
                Flow::Break => break,
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Next | Flow::Continue => {}
            }
            self.exec_stmt(step)?;
        }
        Ok(Flow::Next)
    }

    fn exec_if(&mut self, i: &IfStmt) -> Res<Flow> {
        if self.eval_bool(&i.cond)? {
            self.exec_block(&i.then_block)
        } else {
            match &i.else_branch {
                Some(Else::Block(b)) => self.exec_block(b),
                Some(Else::If(nested)) => self.exec_if(nested),
                None => Ok(Flow::Next),
            }
        }
    }

    fn assign(&mut self, target: &Expr, value: &Expr) -> Res<()> {
        match &target.kind {
            ExprKind::Paren { expr } => self.assign(expr, value),
            ExprKind::Name { ident } => {
                let v = self.eval(value)?;
                if let Some(slot) = self.find_local_mut(&ident.name) {
                    *slot = Some(v);
                    return Ok(());
                }
                match self.lookup(&ident.name) {
                    Some(Entity::Global(slot)) => {
                        self.wm().globals[slot] = Some(v);
                        Ok(())
                    }
                    _ => Err(self.internal(target.span, "cible d'affectation inconnue")),
                }
            }
            ExprKind::Member { object, field } => {
                if let Some(Entity::Scope(t)) = self.entity_of(object) {
                    let v = self.eval(value)?;
                    return match self.p.tables[t].names.get(&field.name) {
                        Some(Entity::Global(slot)) => {
                            self.wm().globals[*slot] = Some(v);
                            Ok(())
                        }
                        _ => Err(self.internal(field.span, "membre de scope non affectable")),
                    };
                }
                let obj = self.eval(object)?;
                let v = self.eval(value)?;
                let Value::Struct(s) = obj else {
                    return Err(self.internal(
                        object.span,
                        "champ d'une valeur qui n'est pas une structure",
                    ));
                };
                let index = self.field_index(&s.borrow(), field)?;
                s.borrow_mut().fields[index] = v;
                Ok(())
            }
            ExprKind::Index { object, index } => {
                let obj = self.eval(object)?;
                let key = self.eval(index)?;
                let v = self.eval(value)?;
                match obj {
                    Value::Array(a) => {
                        let i = self.array_index(&a.borrow(), &key, index.span)?;
                        a.borrow_mut()[i] = v;
                        Ok(())
                    }
                    Value::Map(m) => {
                        // Position cherchée avant d'emprunter en écriture : la clé peut
                        // contenir la map elle-même.
                        let entries = m.borrow().clone();
                        let found = map_position(&entries, &key);
                        let mut m = m.borrow_mut();
                        match found {
                            Some(i) => m[i].1 = v,
                            None => m.push((key, v)),
                        }
                        Ok(())
                    }
                    _ => Err(self.internal(object.span, "`[]` sur une valeur non indexable")),
                }
            }
            _ => Err(self.internal(target.span, "cible d'affectation invalide")),
        }
    }

    fn field_index(&self, s: &StructValue, field: &Ident) -> Res<usize> {
        self.p.types.structs[s.id]
            .fields
            .iter()
            .position(|f| *f == field.name)
            .ok_or_else(|| self.internal(field.span, "champ inconnu"))
    }

    fn array_index(&self, items: &[Value], key: &Value, span: Span) -> Res<usize> {
        let Value::Int(i) = key else {
            return Err(self.internal(span, "index de tableau non entier"));
        };
        usize::try_from(*i)
            .ok()
            .filter(|&i| i < items.len())
            .ok_or_else(|| {
                self.error_with(
                    Diagnostic::new(
                        &codes::INDEX_OUT_OF_BOUNDS,
                        span,
                        format!("index {i} hors du tableau ({} élément(s))", items.len()),
                    )
                    .expected(if items.is_empty() {
                        "aucun index : le tableau est vide".to_string()
                    } else {
                        format!("un index entre 0 et {}", items.len() - 1)
                    })
                    .received(i.to_string()),
                )
            })
    }

    // ---------------------------------------------------------------- expressions

    fn eval_bool(&mut self, cond: &Expr) -> Res<bool> {
        match self.eval(cond)? {
            Value::Bool(b) => Ok(b),
            _ => Err(self.internal(cond.span, "condition non booléenne")),
        }
    }

    fn eval(&mut self, expr: &Expr) -> Res<Value> {
        match &expr.kind {
            ExprKind::Int { text } => text
                .parse()
                .map(Value::Int)
                .map_err(|_| self.internal(expr.span, "entier invalide")),
            ExprKind::Float { text } => text
                .parse()
                .map(Value::Float)
                .map_err(|_| self.internal(expr.span, "décimal invalide")),
            ExprKind::Str { value } => Ok(Value::Str(value.as_str().into())),
            ExprKind::Bool { value } => Ok(Value::Bool(*value)),
            ExprKind::Paren { expr } => self.eval(expr),
            ExprKind::SelfRef => self
                .find_local(SELF_LOCAL)
                .cloned()
                .flatten()
                .ok_or_else(|| self.internal(expr.span, "`self` hors d'une méthode")),
            ExprKind::Name { ident } => {
                if let Some(local) = self.find_local(&ident.name) {
                    return local
                        .clone()
                        .ok_or_else(|| self.unset(&ident.name, expr.span));
                }
                match self.lookup(&ident.name) {
                    Some(Entity::Global(slot)) => self.read_global(slot, expr.span),
                    _ => Err(self.internal(expr.span, "nom qui n'est pas une valeur")),
                }
            }
            ExprKind::Member { object, field } => {
                match self.entity_of(object) {
                    Some(Entity::Scope(t)) => {
                        return match self.p.tables[t].names.get(&field.name) {
                            Some(Entity::Global(slot)) => self.read_global(*slot, expr.span),
                            _ => Err(self
                                .internal(field.span, "membre de scope qui n'est pas une valeur")),
                        };
                    }
                    Some(Entity::Enum(id)) => {
                        return self.p.types.enums[id]
                            .variants
                            .iter()
                            .position(|v| *v == field.name)
                            .map(|variant| Value::Enum { id, variant })
                            .ok_or_else(|| self.internal(field.span, "variante inconnue"));
                    }
                    _ => {}
                }
                match self.eval(object)? {
                    Value::Struct(s) => {
                        let s = s.borrow();
                        let i = self.field_index(&s, field)?;
                        Ok(s.fields[i].clone())
                    }
                    _ => Err(self.internal(object.span, "membre d'une valeur sans membres")),
                }
            }
            ExprKind::Index { object, index } => {
                let obj = self.eval(object)?;
                let key = self.eval(index)?;
                match obj {
                    Value::Array(a) => {
                        let a = a.borrow();
                        let i = self.array_index(&a, &key, index.span)?;
                        Ok(a[i].clone())
                    }
                    Value::Map(m) => m
                        .borrow()
                        .clone()
                        .into_iter()
                        .find(|(k, _)| equal(k, &key))
                        .map(|(_, v)| v)
                        .ok_or_else(|| {
                            self.error_with(
                                Diagnostic::new(
                                    &codes::MISSING_MAP_KEY,
                                    index.span,
                                    format!(
                                        "la clé {} n'existe pas dans la map",
                                        quoted(&key, &self.p.types)
                                    ),
                                )
                                .suggestion(
                                    "lui donner une valeur avant de la lire : `map[clé] = valeur;`",
                                ),
                            )
                        }),
                    _ => Err(self.internal(object.span, "`[]` sur une valeur non indexable")),
                }
            }
            ExprKind::Array { items } => {
                let values = items
                    .iter()
                    .map(|i| self.eval(i))
                    .collect::<Res<Vec<_>>>()?;
                Ok(Value::new_array(values))
            }
            ExprKind::StructLit { path, fields } => {
                let Some(Entity::Struct(id)) = self.resolve_path(path) else {
                    return Err(
                        self.internal(path.span, "littéral sur autre chose qu'une structure")
                    );
                };
                // Les valeurs sont calculées dans l'ordre où elles sont écrites.
                let mut given = Vec::with_capacity(fields.len());
                for f in fields {
                    given.push((f.name.name.as_str(), self.eval(&f.value)?));
                }
                let names = &self.p.types.structs[id].fields;
                let mut values = Vec::with_capacity(names.len());
                for name in names {
                    match given.iter().position(|(n, _)| n == name) {
                        Some(i) => values.push(given.swap_remove(i).1),
                        None => return Err(self.internal(path.span, "champ manquant")),
                    }
                }
                Ok(Value::Struct(std::sync::Arc::new(value::ValueCell::new(
                    StructValue { id, fields: values },
                ))))
            }
            ExprKind::Call { callee, args } => {
                let job = self.prepare_call(callee, args)?;
                let called_at = (self.frame().file, expr.span);
                self.invoke(job, called_at)
            }
            ExprKind::Start { call } => {
                let ExprKind::Call { callee, args } = &call.kind else {
                    return Err(self.internal(call.span, "`start` sans appel"));
                };
                let job = self.prepare_call(callee, args)?;
                let called_at = (self.frame().file, expr.span);
                self.spawn(job, called_at)
            }
            ExprKind::Listen { event, handler } => {
                let Some(Entity::Func(func)) = self.resolve_path(handler) else {
                    return Err(
                        self.internal(handler.span, "abonnement à autre chose qu'une fonction")
                    );
                };
                let w = self.wm();
                w.listeners.push(ListenerSlot {
                    event: path_text(event),
                    func,
                    active: true,
                });
                Ok(Value::Listener(w.listeners.len() - 1))
            }
            ExprKind::Await { expr: inner } => match self.eval(inner)? {
                Value::Task(t) => self.await_task(t, expr.span),
                _ => {
                    Err(self.internal(inner.span, "`await` sur une valeur qui n'est pas une tâche"))
                }
            },
            ExprKind::Unary { op, expr: inner } => {
                let v = self.eval(inner)?;
                match (op, v) {
                    (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
                    (UnaryOp::Neg, Value::Int(i)) => i
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| self.overflow(expr.span, "-")),
                    _ => Err(self.internal(expr.span, "opérateur unaire impossible")),
                }
            }
            ExprKind::Binary { op, lhs, rhs } => self.eval_binary(*op, lhs, rhs, expr.span),
            ExprKind::Is { expr: inner, ty } => {
                let v = self.eval(inner)?;
                let kind = self.kind_of(ty)?;
                Ok(Value::Bool(value_is(&v, &kind)))
            }
            ExprKind::Error => Err(self.internal(expr.span, "expression invalide")),
        }
    }

    fn kind_of(&self, ty: &TypeRef) -> Res<Kind> {
        if let [seg] = ty.path.segments.as_slice() {
            let builtin = match seg.name.as_str() {
                "Int" => Some(Kind::Int),
                "Float" => Some(Kind::Float),
                "Bool" => Some(Kind::Bool),
                "String" => Some(Kind::String),
                "Array" => Some(Kind::Array),
                "Map" => Some(Kind::Map),
                "Task" => Some(Kind::Task),
                "Listener" => Some(Kind::Listener),
                "Char" | "Byte" | "Bytes" | "DateTime" => Some(Kind::NoValue),
                _ => None,
            };
            if let Some(k) = builtin {
                return Ok(k);
            }
        }
        match self.resolve_path(&ty.path) {
            Some(Entity::Struct(id)) => Ok(Kind::Struct(id)),
            Some(Entity::Enum(id)) => Ok(Kind::Enum(id)),
            _ => Err(self.internal(ty.span, "type testé inconnu")),
        }
    }

    fn overflow(&self, span: Span, op: &str) -> Stop {
        self.error_with(
            Diagnostic::new(
                &codes::RUNTIME_INTEGER_OVERFLOW,
                span,
                format!("`{op}` dépasse la plage de `Int` (64 bits signé)"),
            )
            .suggestion("utiliser `Float` pour de très grandes valeurs"),
        )
    }

    fn eval_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span) -> Res<Value> {
        use BinaryOp::*;
        // `&&` et `||` n'évaluent leur droite que si nécessaire.
        if matches!(op, And | Or) {
            let l = self.eval_bool(lhs)?;
            if (op == And && !l) || (op == Or && l) {
                return Ok(Value::Bool(l));
            }
            return self.eval_bool(rhs).map(Value::Bool);
        }
        let l = self.eval(lhs)?;
        let r = self.eval(rhs)?;
        let symbol = match op {
            Eq => return Ok(Value::Bool(equal(&l, &r))),
            NotEq => return Ok(Value::Bool(!equal(&l, &r))),
            Lt => "<",
            Gt => ">",
            Le => "<=",
            Ge => ">=",
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Rem => "%",
            And | Or => unreachable!("traités plus haut"),
        };
        // Concaténation (DECISIONS D22).
        if op == Add && (matches!(l, Value::Str(_)) || matches!(r, Value::Str(_))) {
            let mut s = display(&l, &self.p.types);
            s.push_str(&display(&r, &self.p.types));
            return Ok(Value::Str(s.into()));
        }
        match (&l, &r) {
            (Value::Int(a), Value::Int(b)) => {
                let (a, b) = (*a, *b);
                let v = match op {
                    Lt => return Ok(Value::Bool(a < b)),
                    Gt => return Ok(Value::Bool(a > b)),
                    Le => return Ok(Value::Bool(a <= b)),
                    Ge => return Ok(Value::Bool(a >= b)),
                    Add => a.checked_add(b),
                    Sub => a.checked_sub(b),
                    Mul => a.checked_mul(b),
                    Div | Rem if b == 0 => {
                        return Err(self.error_with(
                            Diagnostic::new(
                                &codes::DIVISION_BY_ZERO,
                                rhs.span,
                                format!("`{symbol}` par zéro"),
                            )
                            .suggestion("vérifier le diviseur avant le calcul"),
                        ))
                    }
                    Div => a.checked_div(b),
                    Rem => a.checked_rem(b),
                    _ => return Err(self.internal(span, "opérateur entier")),
                };
                v.map(Value::Int).ok_or_else(|| self.overflow(span, symbol))
            }
            // Promotion Int → Float dans un calcul (DECISIONS D23) ; règles IEEE 754.
            (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
                let (a, b) = (as_f64(&l), as_f64(&r));
                Ok(match op {
                    Lt => Value::Bool(a < b),
                    Gt => Value::Bool(a > b),
                    Le => Value::Bool(a <= b),
                    Ge => Value::Bool(a >= b),
                    Add => Value::Float(a + b),
                    Sub => Value::Float(a - b),
                    Mul => Value::Float(a * b),
                    Div => Value::Float(a / b),
                    Rem => Value::Float(a % b),
                    _ => return Err(self.internal(span, "opérateur décimal")),
                })
            }
            _ => Err(self.internal(span, "opérandes incompatibles")),
        }
    }
}

fn map_position(entries: &[(Value, Value)], key: &Value) -> Option<usize> {
    entries.iter().position(|(k, _)| equal(k, key))
}

fn as_f64(v: &Value) -> f64 {
    match v {
        Value::Int(i) => *i as f64,
        Value::Float(f) => *f,
        _ => f64::NAN,
    }
}

fn quoted(v: &Value, names: &TypeNames) -> String {
    match v {
        Value::Str(s) => format!("{:?}", &**s),
        other => display(other, names),
    }
}

fn value_is(v: &Value, kind: &Kind) -> bool {
    match (v, kind) {
        (Value::Int(_), Kind::Int)
        | (Value::Float(_), Kind::Float)
        | (Value::Bool(_), Kind::Bool)
        | (Value::Str(_), Kind::String)
        | (Value::Array(_), Kind::Array)
        | (Value::Map(_), Kind::Map)
        | (Value::Task(_), Kind::Task)
        | (Value::Listener(_), Kind::Listener) => true,
        (Value::Struct(s), Kind::Struct(id)) => s.borrow().id == *id,
        (Value::Enum { id, .. }, Kind::Enum(k)) => id == k,
        _ => false,
    }
}

/// Valeur d'une variable déclarée sans valeur : un tableau ou une map simples démarrent vides,
/// toute autre variable reste sans valeur jusqu'à sa première affectation (DECISIONS D31).
fn default_value(ty: &TypeDecl) -> Option<Value> {
    let [single] = ty.types.as_slice() else {
        return None;
    };
    let [seg] = single.path.segments.as_slice() else {
        return None;
    };
    match seg.name.as_str() {
        "Array" => Some(Value::new_array(Vec::new())),
        "Map" => Some(Value::new_map()),
        _ => None,
    }
}

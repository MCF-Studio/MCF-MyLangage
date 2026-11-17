//! Registre des identifiants de diagnostics.
//!
//! Plages : 00xx lexical, 01xx syntaxe, 02xx sémantique/types, 03xx sécurité (à venir),
//! 04xx exécution (`mcf run`), 05xx conseils pour apprendre (jamais bloquants),
//! 06xx fichiers de configuration `.cfg`.

use crate::{Category, Severity};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Code {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub severity: Severity,
    pub summary: &'static str,
}

macro_rules! codes {
    ($($ident:ident = $id:literal, $cat:ident, $sev:ident, $summary:literal;)*) => {
        $(
            pub static $ident: Code = Code {
                id: $id,
                name: stringify!($ident),
                category: Category::$cat,
                severity: Severity::$sev,
                summary: $summary,
            };
        )*
        /// Tous les diagnostics connus, pour l'autodescription (`mcf describe errors`).
        pub static ALL: &[&Code] = &[$(&$ident),*];
    };
}

codes! {
    UNEXPECTED_CHARACTER = "MCF0001", Lexical, Error, "Caractère qui ne peut commencer aucun token.";
    UNTERMINATED_STRING = "MCF0002", Lexical, Error, "Chaîne non fermée avant la fin du fichier.";
    UNTERMINATED_BLOCK_COMMENT = "MCF0003", Lexical, Error, "Commentaire /* non fermé.";
    INVALID_ESCAPE = "MCF0004", Lexical, Error, "Séquence d'échappement inconnue dans une chaîne.";
    INVALID_LOCAL_IDENTIFIER = "MCF0005", Lexical, Error, "Un identifiant local doit être `_` suivi d'une lettre.";
    INVALID_NUMBER_LITERAL = "MCF0006", Lexical, Error, "Littéral numérique mal formé.";
    NEWLINE_IN_STRING = "MCF0007", Lexical, Error, "Retour à la ligne dans une chaîne (utiliser \\n).";
    RESERVED_KEYWORD = "MCF0008", Lexical, Error, "Mot réservé utilisé comme identifiant.";

    UNEXPECTED_TOKEN = "MCF0100", Syntax, Error, "Token inattendu à cet endroit.";
    MISSING_SEMICOLON = "MCF0101", Syntax, Error, "Instruction non terminée par `;`.";
    EXPECTED_IDENTIFIER = "MCF0102", Syntax, Error, "Un identifiant était attendu.";
    LOCAL_DECLARATION_AT_MODULE_LEVEL = "MCF0103", Syntax, Error, "Déclaration préfixée `_` hors d'une fonction : une globale s'écrit sans `_`.";
    GLOBAL_DECLARATION_IN_BLOCK = "MCF0104", Syntax, Error, "Déclaration sans `_` dans un bloc : une locale doit commencer par `_`.";
    PARAMETER_MUST_BE_LOCAL = "MCF0105", Syntax, Error, "Un paramètre doit commencer par `_`.";
    TOO_MANY_TYPES = "MCF0106", Type, Error, "DualType : deux types maximum.";
    EXPECTED_EXPRESSION = "MCF0108", Syntax, Error, "Une expression était attendue.";
    EXPECTED_TYPE = "MCF0109", Syntax, Error, "Un nom de type était attendu.";
    MISSING_RETURN_TYPE = "MCF0111", Syntax, Error, "Le type de retour d'une fonction est obligatoire (`: Void` si aucun).";
    UNNECESSARY_SEMICOLON = "MCF0112", Syntax, Warning, "Point-virgule inutile (après un bloc ou instruction vide).";
    CHAINED_COMPARISON = "MCF0113", Syntax, Error, "Comparaisons enchaînées interdites : utiliser `&&`.";
    INVALID_ASSIGNMENT_TARGET = "MCF0114", Syntax, Error, "Le membre gauche d'une affectation doit être une variable, un membre ou un index.";
    STATEMENT_OUTSIDE_FUNCTION = "MCF0115", Syntax, Error, "Instruction exécutable hors d'une fonction ou d'un gestionnaire `on`.";
    DECLARATION_NOT_ALLOWED_HERE = "MCF0116", Syntax, Error, "Déclaration (func, scope, struct, enum, event, import, on) interdite dans un bloc.";
    MISSING_THEN = "MCF0117", Syntax, Error, "`if (...)` doit être suivi de `then`.";
    MISSING_DO = "MCF0118", Syntax, Error, "Une boucle doit être suivie de `do`.";
    UNCLOSED_DELIMITER = "MCF0119", Syntax, Error, "Délimiteur ouvrant sans fermeture.";
    INVALID_FIELD_NAME = "MCF0120", Syntax, Error, "Un champ de structure s'écrit sans `_`.";
    START_REQUIRES_CALL = "MCF0121", Syntax, Error, "`start` doit être suivi d'un appel de fonction.";

    UNDECLARED_NAME = "MCF0200", Semantic, Error, "Nom utilisé sans déclaration accessible.";
    DUPLICATE_DECLARATION = "MCF0201", Semantic, Error, "Nom déjà déclaré dans le même scope.";
    UNKNOWN_TYPE = "MCF0202", Type, Error, "Type inconnu.";
    DUPLICATE_DUAL_TYPE = "MCF0203", Type, Error, "DualType : les deux types doivent être distincts.";
    SHADOWED_LOCAL = "MCF0204", Semantic, Warning, "Une variable locale en masque une autre d'un bloc parent.";
    UNDECLARED_EVENT = "MCF0205", Semantic, Error, "Événement non déclaré.";
    UNRESOLVED_IMPORT = "MCF0206", Import, Error, "Import introuvable dans le projet.";
    UNKNOWN_MEMBER = "MCF0207", Semantic, Error, "Membre absent du scope.";
    IMPORT_CONFLICT = "MCF0208", Import, Error, "Import ambigu ou en collision avec une déclaration.";
    EVENT_ARITY_MISMATCH = "MCF0209", Semantic, Error, "Nombre de données d'événement incorrect.";
    ARGUMENT_COUNT_MISMATCH = "MCF0210", Semantic, Error, "Nombre d'arguments incorrect.";
    LOOP_CONTROL_OUTSIDE_LOOP = "MCF0211", Semantic, Error, "`break` ou `continue` hors d'une boucle.";
    HANDLER_MISMATCH = "MCF0213", Semantic, Error, "La fonction abonnée ne correspond pas aux données de l'événement.";
    NOT_ASSIGNABLE = "MCF0212", Semantic, Error, "Une fonction, un scope ou une fonction native ne peut pas recevoir de valeur.";

    TYPE_MISMATCH = "MCF0220", Type, Error, "Le type de la valeur ne correspond pas au type attendu.";
    DUAL_TYPE_NOT_NARROWED = "MCF0221", Type, Error, "Valeur DualType utilisée sans vérifier son type actif (`is`).";
    IMPOSSIBLE_TYPE_TEST = "MCF0222", Type, Error, "Test `is` vers un type que la valeur ne peut jamais avoir.";
    TYPE_TEST_ALWAYS_TRUE = "MCF0223", Type, Warning, "Test `is` toujours vrai.";
    MISSING_RETURN_VALUE = "MCF0224", Type, Error, "`return;` sans valeur dans une fonction qui doit retourner une valeur.";
    UNEXPECTED_RETURN_VALUE = "MCF0225", Type, Error, "`return` avec une valeur dans une fonction `Void`.";
    MISSING_RETURN = "MCF0226", Type, Error, "Un chemin de la fonction se termine sans `return`.";
    VOID_NOT_ALLOWED = "MCF0227", Type, Error, "`Void` n'est autorisé que comme type de retour seul.";
    WRONG_TYPE_ARGUMENTS = "MCF0228", Type, Error, "Nombre d'arguments de type générique incorrect.";
    INTEGER_OVERFLOW = "MCF0229", Type, Error, "Entier hors de la plage de `Int` (64 bits signé).";
    INVALID_OPERANDS = "MCF0230", Type, Error, "Opérateur impossible avec ces types.";
    NOT_CALLABLE = "MCF0231", Type, Error, "Cette valeur n'est pas une fonction.";
    NOT_A_VALUE = "MCF0232", Type, Error, "Une fonction, un scope ou un type utilisé comme une valeur.";

    MISSING_FIELD = "MCF0240", Type, Error, "Champ obligatoire absent d'un littéral de structure.";
    UNKNOWN_FIELD = "MCF0241", Type, Error, "La structure ne possède pas ce champ.";
    DUPLICATE_FIELD = "MCF0242", Type, Error, "Champ déclaré ou initialisé deux fois.";
    UNKNOWN_VARIANT = "MCF0243", Type, Error, "L'énumération ne possède pas cette variante.";
    RECURSIVE_STRUCT = "MCF0244", Type, Error, "Structure qui se contient elle-même : taille infinie.";
    NOT_A_TYPE = "MCF0245", Type, Error, "Ce nom ne désigne pas un type.";
    NOT_A_STRUCT = "MCF0246", Type, Error, "Littéral `Nom { ... }` sur autre chose qu'une structure.";
    SELF_OUTSIDE_METHOD = "MCF0247", Semantic, Error, "`self` utilisé hors d'une méthode de structure.";

    INVALID_ENTRY_POINT = "MCF0250", Semantic, Error, "`main` à la racine d'un fichier doit être `func main(): Void` sans paramètre.";
    DUPLICATE_ENTRY_POINT = "MCF0251", Semantic, Error, "Plusieurs points d'entrée `main` dans le même projet.";

    MISSING_ENTRY_POINT = "MCF0400", Runtime, Error, "Projet exécuté sans `func main(): Void`.";
    DIVISION_BY_ZERO = "MCF0401", Runtime, Error, "Division ou reste entier par zéro.";
    RUNTIME_INTEGER_OVERFLOW = "MCF0402", Runtime, Error, "Calcul entier hors de la plage de `Int` (64 bits signé).";
    INDEX_OUT_OF_BOUNDS = "MCF0403", Runtime, Error, "Index hors du tableau.";
    MISSING_MAP_KEY = "MCF0404", Runtime, Error, "Lecture d'une clé absente d'une map.";
    UNINITIALIZED_VARIABLE = "MCF0405", Runtime, Error, "Variable lue avant d'avoir reçu une valeur.";
    STACK_OVERFLOW = "MCF0406", Runtime, Error, "Récursion trop profonde.";
    INVALID_ARGUMENT = "MCF0407", Runtime, Error, "Argument refusé par une fonction native.";
    OUTPUT_ERROR = "MCF0408", Runtime, Error, "Écriture impossible sur la sortie.";
    EMPTY_COLLECTION = "MCF0409", Runtime, Error, "Retrait dans un tableau vide.";
    TASK_CANCELLED = "MCF0410", Runtime, Error, "`await` sur une tâche annulée.";
    DEADLOCK = "MCF0411", Runtime, Error, "Toutes les tâches s'attendent : le programme ne peut plus avancer.";
    TASK_START_FAILED = "MCF0412", Runtime, Error, "Le système refuse de démarrer une nouvelle tâche.";
    CONFIG_LOAD_FAILED = "MCF0413", Runtime, Error, "Fichier de configuration illisible ou non conforme au démarrage ou dans `loadConfig`.";
    CONFIG_MODIFIED = "MCF0414", Runtime, Error, "Modification d'une valeur venue d'une déclaration `config` (lecture seule).";
    INTERNAL_RUNTIME_ERROR = "MCF0499", Runtime, Error, "Erreur interne de l'interpréteur (à signaler).";

    LOOP_WITHOUT_PAUSE_IN_TASK = "MCF0500", Advice, Hint, "Boucle sans pause dans une fonction lancée en tâche : les autres tâches attendent qu'elle finisse.";
    ENDLESS_LOOP_WITHOUT_PAUSE = "MCF0501", Advice, Hint, "`while (true)` qui ne s'arrête jamais et ne laisse jamais la main : le programme se fige.";
    FLOAT_EQUALITY = "MCF0502", Advice, Hint, "`==` entre décimaux : les arrondis rendent l'égalité exacte trompeuse.";
    COMPARISON_WITH_BOOL_LITERAL = "MCF0503", Advice, Hint, "Comparaison avec `true` / `false` inutile.";

    CONFIG_SYNTAX = "MCF0600", Config, Error, "Fichier `.cfg` qui n'est pas du JSON valide (commentaires `//` et `/* */` permis).";
    CONFIG_DUPLICATE_KEY = "MCF0601", Config, Error, "Clé présente deux fois dans le même objet d'un `.cfg`.";
    CONFIG_LIMIT_EXCEEDED = "MCF0602", Config, Error, "Fichier `.cfg` trop gros ou trop imbriqué.";
    CONFIG_FILE_NOT_FOUND = "MCF0610", Config, Error, "Fichier `.cfg` introuvable dans le projet.";
    CONFIG_INVALID_PATH = "MCF0611", Config, Error, "Chemin de configuration invalide : relatif, avec `/`, terminé par `.cfg`.";
    CONFIG_UNSUPPORTED_TYPE = "MCF0612", Config, Error, "Type qui ne peut pas venir d'un fichier `.cfg`.";
    CONFIG_AMBIGUOUS_TYPE = "MCF0613", Config, Error, "DualType dont les deux types s'écrivent pareil en JSON.";
    CONFIG_READ_ONLY = "MCF0614", Config, Error, "Une configuration déclarée par `config` se lit seulement.";
    LOAD_CONFIG_NEEDS_TYPE = "MCF0615", Config, Error, "`loadConfig` doit initialiser une variable dont le type est écrit.";
    CONFIG_WRONG_TYPE = "MCF0620", Config, Error, "Valeur JSON qui ne correspond pas au type MCF attendu.";
    CONFIG_MISSING_KEY = "MCF0621", Config, Error, "Champ de la structure absent du `.cfg`.";
    CONFIG_UNKNOWN_KEY = "MCF0622", Config, Error, "Clé du `.cfg` qui ne correspond à aucun champ.";
    CONFIG_UNKNOWN_VARIANT = "MCF0623", Config, Error, "Chaîne qui ne nomme aucune variante de l'énumération.";
    CONFIG_NUMBER_OUT_OF_RANGE = "MCF0624", Config, Error, "Nombre hors de la plage de `Int` ou de `Float`.";
}

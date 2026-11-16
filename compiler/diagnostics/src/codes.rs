//! Registre des identifiants de diagnostics.
//!
//! Plages : 00xx lexical, 01xx syntaxe, 02xx sémantique/types (à venir), 03xx sécurité.

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
    DECLARATION_NOT_ALLOWED_HERE = "MCF0116", Syntax, Error, "Déclaration (func, scope, event, import, on) interdite dans un bloc.";
    MISSING_THEN = "MCF0117", Syntax, Error, "`if (...)` doit être suivi de `then`.";
    MISSING_DO = "MCF0118", Syntax, Error, "Une boucle doit être suivie de `do`.";
    UNCLOSED_DELIMITER = "MCF0119", Syntax, Error, "Délimiteur ouvrant sans fermeture.";
}

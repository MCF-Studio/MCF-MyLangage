//! Serveur local de l'aperçu interactif (`mcf view --serve`).
//!
//! Écoute **127.0.0.1 seulement**, une requête à la fois. Chaque adresse porte une clé
//! aléatoire (`?k=…`) : une autre page web ouverte dans le navigateur ne peut ni lire la vue ni
//! envoyer d'événement. C'est un outil de développement, pas un serveur de production.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use mcf_runtime::ViewSession;
use mcf_view::Event;

use crate::live;

#[derive(serde::Deserialize)]
struct EventRequest {
    path: Vec<usize>,
    event: Event,
    text: Option<String>,
}

fn random_key() -> String {
    let mut key = String::new();
    for _ in 0..2 {
        key.push_str(&format!(
            "{:016x}",
            RandomState::new().build_hasher().finish()
        ));
    }
    key
}

pub fn serve(session: &mut ViewSession<'_>, title: &str, port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let key = random_key();
    let addr = listener.local_addr()?;
    eprintln!("aperçu interactif : http://{addr}/?k={key}");
    eprintln!("(Ctrl+C pour arrêter)");
    for stream in listener.incoming().flatten() {
        let _ = handle(stream, session, title, &key);
    }
    Ok(())
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn error_json(e: &mcf_runtime::RuntimeError) -> String {
    let d = &e.diagnostic;
    eprintln!("erreur[{}] {}: {}", d.code.id, d.code.name, d.message);
    serde_json::json!({ "error": format!("{} : {}", d.code.id, d.message) }).to_string()
}

fn view_json(session: &mut ViewSession<'_>) -> String {
    match session.view() {
        Ok(tree) => serde_json::json!({ "tree": live::live_json(&tree, false) }).to_string(),
        Err(e) => error_json(&e),
    }
}

fn handle(
    mut stream: TcpStream,
    session: &mut ViewSession<'_>,
    title: &str,
    key: &str,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if query != format!("k={key}") {
        return respond(&mut stream, "403 Forbidden", "text/plain", "clé invalide");
    }
    match (method, path) {
        ("GET", "/") => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            &live::live_page(title, key),
        ),
        ("GET", "/view") => {
            let body = view_json(session);
            respond(&mut stream, "200 OK", "application/json", &body)
        }
        ("POST", "/event") => {
            // Un événement tient en quelques centaines d'octets : corps limité.
            if length > 64 * 1024 {
                return respond(
                    &mut stream,
                    "413 Payload Too Large",
                    "text/plain",
                    "trop gros",
                );
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body)?;
            let body = match serde_json::from_slice::<EventRequest>(&body) {
                Ok(req) => match session.dispatch(&req.path, req.event, req.text.as_deref()) {
                    Ok(_) => {
                        let printed = session.take_output();
                        if !printed.is_empty() {
                            print!("{printed}");
                        }
                        view_json(session)
                    }
                    Err(e) => error_json(&e),
                },
                Err(_) => serde_json::json!({ "error": "événement illisible" }).to_string(),
            };
            respond(&mut stream, "200 OK", "application/json", &body)
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", "introuvable"),
    }
}

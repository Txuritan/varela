use {
    crate::{
        data::Database,
        models::proto::{Entity, Story},
    },
    std::collections::BTreeMap,
};

macro_rules! help {
    (bound; $db:ident, $iter:ident, $text:ident, $var:ident, $mem:ident) => {{
        BoundIter::$var(
            $iter.filter(move |(_, s)| any_by_text(&$db.index.$mem, &s.meta.$mem, &$text)),
        )
    }};
    (retain; $db:ident, $stories:ident, $include:ident, $text:ident, $mem:ident) => {{
        $stories.retain(|id| {
            let story = $db.index.stories.get(id).unwrap();

            !(&$include ^ any_by_text(&$db.index.$mem, &story.meta.$mem, &$text))
        });
    }};
}

pub enum Bound {
    Author { include: bool, text: String },
    Origin { include: bool, text: String },
    Pairing { include: bool, text: String },
    Character { include: bool, text: String },
    General { include: bool, text: String },
}

impl Bound {
    const fn author(include: bool, text: String) -> Bound {
        Bound::Author { include, text }
    }

    const fn origin(include: bool, text: String) -> Bound {
        Bound::Origin { include, text }
    }

    const fn character(include: bool, text: String) -> Bound {
        Bound::Character { include, text }
    }
}

pub fn search(database: &Database, text: &str) -> Vec<String> {
    let bounds = parse(text);

    let mut stories = Vec::new();

    let mut bounds_iter = bounds.into_iter();

    if let Some(bound) = bounds_iter.next() {
        let story_iter = database.index.stories.iter();

        let (include, iter) = match bound {
            Bound::Author { include, text } => (
                include,
                help!(bound; database, story_iter, text, Author, authors),
            ),
            Bound::Origin { include, text } => (
                include,
                help!(bound; database, story_iter, text, Origin, origins),
            ),
            Bound::Pairing { include, text } => (
                include,
                help!(bound; database, story_iter, text, Pairing, pairings),
            ),
            Bound::Character { include, text } => (
                include,
                help!(bound; database, story_iter, text, Character, characters),
            ),
            Bound::General { include, text } => (
                include,
                help!(bound; database, story_iter, text, General, generals),
            ),
        };

        first_push(include, &database, &mut stories, iter);
    }

    for bound in bounds_iter {
        match bound {
            Bound::Author { include, text } => {
                help!(retain; database, stories, include, text, authors);
            }
            Bound::Origin { include, text } => {
                help!(retain; database, stories, include, text, origins);
            }
            Bound::Pairing { include, text } => {
                help!(retain; database, stories, include, text, pairings);
            }
            Bound::Character { include, text } => {
                help!(retain; database, stories, include, text, characters);
            }
            Bound::General { include, text } => {
                help!(retain; database, stories, include, text, generals);
            }
        }
    }

    stories
}

fn first_push<'d, I>(include: bool, database: &Database, stories: &mut Vec<String>, ids: I)
where
    I: Iterator<Item = (&'d String, &'d Story)>,
{
    if include {
        for id in ids.map(|(id, _)| id) {
            if !stories.contains(id) {
                stories.push(id.clone());
            }
        }
    } else {
        let ids = ids.map(|(id, _)| id).collect::<Vec<_>>();

        for id in database.index.stories.iter().map(|(id, _)| id) {
            if !ids.contains(&id) {
                stories.push(id.clone());
            }
        }
    }
}

fn any_by_text(full: &BTreeMap<String, Entity>, refs: &[String], text: &str) -> bool {
    refs.iter().map(|id| full.get(id)).any(|a| match a {
        Some(entity) => entity.text == text,
        None => false,
    })
}

#[allow(clippy::while_let_on_iterator)]
fn parse(text: &str) -> Vec<Bound> {
    let mut parts = text.split(',').map(str::trim);

    let mut bounds = Vec::with_capacity(parts.size_hint().0);

    while let Some(mut part) = parts.next() {
        let included = part.starts_with('-');

        if included {
            part = part.trim_start_matches('-');
        }

        if parse_group(
            ["[", "]", "/"],
            &mut bounds,
            &mut parts,
            included,
            &mut part,
        ) {
            continue;
        }

        if parse_group(
            ["(", ")", " & "],
            &mut bounds,
            &mut parts,
            included,
            &mut part,
        ) {
            continue;
        }

        if parse_prefixed(
            ["a:", "author:"],
            Bound::author,
            &mut bounds,
            included,
            &mut part,
        ) {
            continue;
        }

        if parse_prefixed(
            ["c:", "origin:"],
            Bound::origin,
            &mut bounds,
            included,
            &mut part,
        ) {
            continue;
        }

        if parse_prefixed(
            ["c:", "character:"],
            Bound::character,
            &mut bounds,
            included,
            &mut part,
        ) {
            continue;
        }

        bounds.push(Bound::General {
            include: included,
            text: part.to_owned(),
        });
    }

    bounds
}

fn parse_prefixed<B>(
    prefixes: [&str; 2],
    builder: B,
    bounds: &mut Vec<Bound>,
    included: bool,
    part: &mut &str,
) -> bool
where
    B: FnOnce(bool, String) -> Bound,
{
    let [short, long] = prefixes;

    if part.starts_with(short) || part.starts_with(long) {
        let part = part
            .trim_start_matches(short)
            .trim_start_matches(long)
            .to_string();

        bounds.push(builder(included, part));

        true
    } else {
        false
    }
}

fn parse_group<'i, I>(
    symbols: [&str; 3],
    bounds: &mut Vec<Bound>,
    parts: &mut I,
    included: bool,
    part: &mut &str,
) -> bool
where
    I: Iterator<Item = &'i str>,
{
    let [open, close, sep] = symbols;

    if part.starts_with(open) {
        let mut part = part.trim_start_matches(open).to_string();

        part.push_str(sep);

        for mut inner in parts {
            if inner.ends_with(close) {
                inner = inner.trim_end_matches(close);

                part.push_str(inner);

                break;
            }

            part.push_str(inner);
            part.push_str(sep);
        }

        bounds.push(Bound::Pairing {
            include: included,
            text: part,
        });

        true
    } else {
        false
    }
}

enum BoundIter<I, A, O, P, C, G>
where
    A: Iterator<Item = I>,
    O: Iterator<Item = I>,
    P: Iterator<Item = I>,
    C: Iterator<Item = I>,
    G: Iterator<Item = I>,
{
    Author(A),
    Origin(O),
    Pairing(P),
    Character(C),
    General(G),
}

impl<I, A, O, P, C, G> Iterator for BoundIter<I, A, O, P, C, G>
where
    A: Iterator<Item = I>,
    O: Iterator<Item = I>,
    P: Iterator<Item = I>,
    C: Iterator<Item = I>,
    G: Iterator<Item = I>,
{
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            BoundIter::Author(i) => i.next(),
            BoundIter::Origin(i) => i.next(),
            BoundIter::Pairing(i) => i.next(),
            BoundIter::Character(i) => i.next(),
            BoundIter::General(i) => i.next(),
        }
    }
}

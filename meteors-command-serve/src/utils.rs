use {
    common::{
        database::Database,
        models::{
            proto::{Entity, Index, Range, Rating, StoryMeta},
            StoryFull, StoryFullMeta,
        },
        prelude::*,
    },
    std::{convert::TryInto as _, fs::File, io::Read as _},
};

pub fn get_chapter_body(db: &Database, id: &str, chapter: usize) -> Result<String> {
    let story = db
        .index
        .stories
        .get(id)
        .ok_or_else(|| anyhow!("unable to find story in index"))?;

    let path = db.data_path.join(&story.file_name);

    let mut file = File::open(&path)?;

    let mut contents = String::with_capacity(story.length.try_into()?);

    let _ = file.read_to_string(&mut contents)?;

    let range = story.chapters.get(chapter - 1).ok_or_else(|| {
        anyhow!(
            "chapter `{}` not found, chapters: {}",
            chapter,
            story.chapters.len()
        )
    })?;

    Ok(contents
        .get((range.start.try_into()?)..(range.end.try_into()?))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "chapter `{}` not found in chapter index for `{}`",
                chapter,
                id
            )
        })?
        .to_owned())
}

#[allow(clippy::ptr_arg)]
pub fn get_story_full<'i>(db: &Database, id: &'i String) -> Result<(&'i String, StoryFull)> {
    enum Kind {
        Categories,
        Authors,
        Origins,
        Warnings,
        Pairings,
        Characters,
        Generals,
    }

    fn values(index: &Index, meta: &StoryMeta, kind: &Kind) -> Result<Vec<Entity>> {
        let (map, keys) = match kind {
            Kind::Categories => (&index.categories, &meta.categories),
            Kind::Authors => (&index.authors, &meta.authors),
            Kind::Origins => (&index.origins, &meta.origins),
            Kind::Warnings => (&index.warnings, &meta.warnings),
            Kind::Pairings => (&index.pairings, &meta.pairings),
            Kind::Characters => (&index.characters, &meta.characters),
            Kind::Generals => (&index.generals, &meta.generals),
        };

        keys.iter()
            .map(|id| {
                map.get(id)
                    .cloned()
                    .ok_or_else(|| anyhow!("entity with id `{}` does not exist", id))
            })
            .collect::<Result<Vec<_>>>()
    }

    let story_ref = db
        .index
        .stories
        .get(id)
        .ok_or_else(|| anyhow!("story with id `{}` does not exist", id))?;

    let index = &db.index;
    let meta = &story_ref.meta;

    Ok((
        id,
        StoryFull {
            file_name: story_ref.file_name.clone(),
            length: story_ref.length.try_into()?,
            chapters: story_ref
                .chapters
                .iter()
                .map(Range::to_std)
                .collect::<Result<Vec<_>>>()?,
            info: story_ref.info.clone(),
            meta: StoryFullMeta {
                rating: Rating::from(story_ref.meta.rating),
                categories: values(&index, &meta, &Kind::Categories).context("categories")?,
                authors: values(&index, &meta, &Kind::Authors).context("authors")?,
                origins: values(&index, &meta, &Kind::Origins).context("origins")?,
                warnings: values(&index, &meta, &Kind::Warnings).context("warnings")?,
                pairings: values(&index, &meta, &Kind::Pairings).context("pairings")?,
                characters: values(&index, &meta, &Kind::Characters).context("characters")?,
                generals: values(&index, &meta, &Kind::Generals).context("generals")?,
            },
        },
    ))
}

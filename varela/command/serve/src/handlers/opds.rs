use common::models::FileKind;

use {
    crate::{templates::pages::opds::OpdsFeed, utils},
    common::{database::Database, models::Existing, prelude::*},
    enrgy::{http::headers::CONTENT_TYPE, web, HttpResponse},
    std::str::FromStr,
};

pub enum CatalogFormat {
    Atom, // opds+1.2
    Html,
    Json, // opds+2.0
}

impl FromStr for CatalogFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "atom" | "xml" => Ok(CatalogFormat::Atom),
            "html" => Ok(CatalogFormat::Html),
            "json" => Ok(CatalogFormat::Json),
            ext => Err(anyhow!("Unknown catalog format: {}", ext)),
        }
    }
}

pub fn catalog(
    db: web::Data<Database>,
    ext: web::ParseParam<"ext", CatalogFormat>,
) -> HttpResponse {
    utils::wrap(|| {
        let mut stories = db
            .index()
            .stories
            .iter()
            .filter(|(_, s)| s.info.kind == FileKind::Epub)
            .map(|(id, _)| {
                utils::get_story_full(&db, id).map(|story| Existing::new(id.clone(), story))
            })
            .collect::<Result<Vec<_>>>()?;

        stories.sort_by(|a, b| a.info.updated.cmp(&b.info.updated));

        let updated = stories
            .first()
            .map(|story| story.info.updated.clone())
            .unwrap_or_else(|| humantime::format_rfc3339(std::time::SystemTime::now()).to_string());

        Ok(HttpResponse::ok()
            .header(CONTENT_TYPE, "application/atom+xml")
            .body(::opal::Template::render_into_string(OpdsFeed::new(
                updated, stories,
            ))?))
    })
}

use crate::errors::{ErrorKind, Result};
use crate::fsshttpb::data::exguid::ExGuid;
use crate::one::property_set::{page_metadata, page_series_node};
use crate::onenote::ParserContext;
use crate::onenote::page::{Page, parse_page};
use crate::onestore::OneStore;
use uuid::Uuid;

/// A series of page.
///
/// See [\[MS-ONE\] 1.3.2] and [\[MS-ONE\] 2.2.18].
///
/// [\[MS-ONE\] 1.3.2]: https://docs.microsoft.com/en-us/openspecs/office_file_formats/ms-one/2dd687ac-f36b-4723-b959-4d60c8a90ca9
/// [\[MS-ONE\] 2.2.18]: https://docs.microsoft.com/en-us/openspecs/office_file_formats/ms-one/e2957d3b-a2a8-4756-8662-4e67fefa9f4e
#[derive(Clone, Debug)]
pub struct PageSeries {
    pages: Vec<Page>,
    unavailable_pages: Vec<UnavailablePage>,
}

impl PageSeries {
    /// The pages contained in this page series.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Pages the series lists whose content the file does not contain.
    pub fn unavailable_pages(&self) -> &[UnavailablePage] {
        &self.unavailable_pages
    }
}

/// A page the series lists whose content the file does not contain.
///
/// Seen in sections OneNote keeps in OneDrive: the storage index maps the page's cell to nothing
/// until a OneNote client writes the page into the file. Only what the section itself caches about
/// the page is known: its title, level and creation time.
#[derive(Clone, Debug)]
pub struct UnavailablePage {
    position: usize,
    title: Option<String>,
    level: i32,
    created_time: Option<time::UtcDateTime>,
    space_id: (Uuid, u32),
}

impl UnavailablePage {
    /// The page's position among all pages of its series, including the unavailable ones.
    pub fn position(&self) -> usize {
        self.position
    }

    /// The page title OneNote caches in the section.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The page's level (1 for a top-level page, 2+ for sub-pages).
    pub fn level(&self) -> i32 {
        self.level
    }

    /// When the page was created.
    pub fn created_time(&self) -> Option<time::UtcDateTime> {
        self.created_time
    }

    /// The id of the page's object space (see [`Page::space_id`]).
    pub fn space_id(&self) -> (Uuid, u32) {
        self.space_id
    }
}

pub(crate) fn parse_page_series(
    id: ExGuid,
    store: &(impl OneStore + ?Sized),
    ctx: &mut ParserContext,
) -> Result<PageSeries> {
    let object = store
        .data_root()
        .get_object(id)
        .ok_or_else(|| ErrorKind::MalformedOneNoteData("page series object is missing".into()))?;
    let data = page_series_node::parse(object)?;

    // A page series can list a page whose object space the file does not contain (seen in a
    // section saved by OneNote through OneDrive). Skip that page with a warning instead of
    // failing the whole section: the other pages are intact.
    // Such a page is still listed in OneNote: report it as unavailable, with the title and
    // creation time the series keeps for each of its pages.
    let mut pages = Vec::with_capacity(data.page_spaces.len());
    let mut unavailable_pages = Vec::new();
    for (position, page_space_id) in data.page_spaces.iter().copied().enumerate() {
        let space_id = (page_space_id.1.guid.0, page_space_id.1.value);
        let Some(page_space) = store.object_space(page_space_id) else {
            warn!(ctx, "page space {page_space_id:?} is missing, page skipped");
            // The section keeps each page's metadata (its title, level, creation time) as an
            // object whose id is the page's space id XORed with the seed of [MS-ONE] 2.2.18
            // (seen in OneDrive sections: {96B62519-...},247 -> {B41EE528-...},247).
            let metadata = store
                .data_root()
                .get_object(page_space_id.1 ^ exguid!({{"22a8c031-3600-42ee-b714-d7acda2435e8"}, 0}))
                .and_then(|object| page_metadata::parse(object).ok());
            if metadata.as_ref().is_some_and(|m| m.is_deleted) {
                continue;
            }
            unavailable_pages.push(UnavailablePage {
                position,
                title: metadata.as_ref().and_then(|m| m.cached_title.clone()),
                level: metadata.as_ref().map(|m| m.page_level).unwrap_or(1),
                created_time: metadata.and_then(|m| m.created_at.try_into().ok()),
                space_id,
            });
            continue;
        };

        let mut page = parse_page(page_space, ctx)?;
        page.space_id = Some(space_id);
        pages.push(page);
    }

    Ok(PageSeries {
        pages,
        unavailable_pages,
    })
}

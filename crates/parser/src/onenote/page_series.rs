use crate::errors::{ErrorKind, Result};
use crate::fsshttpb::data::exguid::ExGuid;
use crate::one::property_set::page_series_node;
use crate::onenote::ParserContext;
use crate::onenote::page::{Page, parse_page};
use crate::onestore::OneStore;

/// A series of page.
///
/// See [\[MS-ONE\] 1.3.2] and [\[MS-ONE\] 2.2.18].
///
/// [\[MS-ONE\] 1.3.2]: https://docs.microsoft.com/en-us/openspecs/office_file_formats/ms-one/2dd687ac-f36b-4723-b959-4d60c8a90ca9
/// [\[MS-ONE\] 2.2.18]: https://docs.microsoft.com/en-us/openspecs/office_file_formats/ms-one/e2957d3b-a2a8-4756-8662-4e67fefa9f4e
#[derive(Clone, Debug)]
pub struct PageSeries {
    pages: Vec<Page>,
}

impl PageSeries {
    /// The pages contained in this page series.
    pub fn pages(&self) -> &[Page] {
        &self.pages
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
    let mut pages = Vec::with_capacity(data.page_spaces.len());
    for page_space_id in data.page_spaces {
        let Some(page_space) = store.object_space(page_space_id) else {
            warn!(ctx, "page space {page_space_id:?} is missing, page skipped");
            continue;
        };

        pages.push(parse_page(page_space, ctx)?);
    }

    Ok(PageSeries { pages })
}

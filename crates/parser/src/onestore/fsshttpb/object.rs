use crate::errors::{ErrorKind, Result};
use crate::fsshttpb::data::exguid::ExGuid;
use crate::fsshttpb::data_element::object_group::ObjectGroupData;
use crate::fsshttpb::packaging::OneStorePackaging;
use crate::onestore::Object;
use crate::onestore::fsshttpb::mapping_table::MappingTable;
use crate::onestore::fsshttpb::object_space::GroupData;
use crate::onestore::shared::jcid::JcId;
use crate::onestore::shared::object_prop_set::ObjectPropSet;
use crate::reader::Reader;
use crate::shared::guid::Guid;
use std::rc::Rc;

#[derive(Debug, Copy, Clone)]
enum Partition {
    Metadata = 4,
    ObjectData = 1,
    FileData = 2,
}

impl<'a> Object {
    pub(crate) fn parse<'b>(
        object_id: ExGuid,
        context_id: ExGuid,
        object_space_id: ExGuid,
        objects: &'b GroupData<'a>,
        packaging: &'a OneStorePackaging,
    ) -> Result<Object> {
        let metadata_object = Object::find_object(object_id, Partition::Metadata, objects)
            .ok_or_else(|| ErrorKind::MalformedOneStoreData("object metadata is missing".into()))?;
        let data_object = Object::find_object(object_id, Partition::ObjectData, objects)
            .ok_or_else(|| ErrorKind::MalformedOneStoreData("object data is missing".into()))?;

        // Parse metadata

        let metadata = if let ObjectGroupData::Object { data, .. } = metadata_object {
            data
        } else {
            return Err(ErrorKind::MalformedOneStoreData(
                "object metadata it not an object".into(),
            )
            .into());
        };

        let jc_id = JcId::parse(&mut Reader::new(metadata.as_slice()))?;

        // Parse data

        let (data, object_refs, referenced_cells) =
            if let ObjectGroupData::Object { group, cells, data } = data_object {
                (data, group, cells)
            } else {
                return Err(ErrorKind::MalformedOneStoreData(
                    "object data it not an object".into(),
                )
                .into());
            };

        let props = ObjectPropSet::parse(&mut Reader::new(data.as_slice()))?;

        // Parse file data

        let file_data = Object::find_blob_id(object_id, objects)?
            .map(|blob_id| {
                packaging
                    .data_element_package
                    .find_blob(blob_id)
                    .ok_or_else(|| ErrorKind::MalformedOneStoreData("blob not found".into()))
            })
            .transpose()?;

        let context_refs: Vec<_> = referenced_cells
            .iter()
            .filter(|id| id.1 == object_space_id)
            .map(|id| id.0)
            .collect();

        let object_space_refs: Vec<_> = referenced_cells
            .iter()
            .filter(|id| id.1 != object_space_id)
            .copied()
            .collect();

        if props.object_ids().len() < object_refs.len() {
            return Err(ErrorKind::MalformedOneStoreData(
                "object ref array sizes do not match".into(),
            )
            .into());
        }

        if props.context_ids().len() + props.object_space_ids().len() != referenced_cells.len() {
            return Err(ErrorKind::MalformedOneStoreData(
                "object space/context array sizes do not match".into(),
            )
            .into());
        }

        // An all-zero CompactId is a null reference (for example an unset author). Null references
        // can be left out of the object's reference list, and pairing both lists by position would
        // then shift every later reference onto the wrong object. When the counts differ, map null
        // ids to the nil ExGuid without consuming a reference.
        let skip_null_ids = props.object_ids().len() != object_refs.len();
        let mut remaining_refs = object_refs.iter().copied();
        let mapping_objects: Vec<_> = props
            .object_ids()
            .iter()
            .copied()
            .map_while(|cid| {
                if skip_null_ids && cid.n == 0 && cid.guid_index == 0 {
                    Some((cid, ExGuid::from_guid(Guid::nil(), 0)))
                } else {
                    remaining_refs.next().map(|id| (cid, id))
                }
            })
            .collect();

        let mapping_contexts = props.context_ids().iter().copied().zip(context_refs);

        let mapping_object_spaces = props
            .object_space_ids()
            .iter()
            .copied()
            .zip(object_space_refs);

        let mapping = MappingTable::from_entries(
            mapping_objects.into_iter().chain(mapping_contexts),
            mapping_object_spaces,
        );

        Ok(Object {
            context_id,
            jc_id,
            props,
            file_data: file_data.clone(),
            mapping: Rc::new(mapping),
        })
    }

    fn find_object<'b>(
        id: ExGuid,
        partition_id: Partition,
        objects: &'b GroupData<'a>,
    ) -> Option<&'b ObjectGroupData> {
        objects.get(&(id, partition_id as u64)).cloned()
    }

    fn find_blob_id(id: ExGuid, objects: &GroupData<'a>) -> Result<Option<ExGuid>> {
        Self::find_object(id, Partition::FileData, objects)
            .map(|object| match object {
                ObjectGroupData::BlobReference { blob, .. } => Ok(*blob),
                _ => {
                    Err(ErrorKind::MalformedOneStoreData("blob object is not a blob".into()).into())
                }
            })
            .transpose()
    }
}

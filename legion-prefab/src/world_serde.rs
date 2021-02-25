use crate::format::EntityUuid;
use crate::registration::ComponentRegistration;
use legion::serialize::{UnknownType, CustomEntitySerializer};
use legion::storage::{ArchetypeIndex, UnknownComponentStorage, UnknownComponentWriter};
use legion::{
    storage::{ComponentTypeId, EntityLayout},
    *,
};
use serde::{Deserializer, Serializer};
use std::{cell::RefCell, collections::HashMap};

pub struct CustomSerializer<'a> {
    pub comp_types: &'a HashMap<ComponentTypeId, ComponentRegistration>,
    pub entity_map: RefCell<&'a mut HashMap<legion::Entity, EntityUuid>>,
}

impl<'a> CustomEntitySerializer for CustomSerializer<'a>{
    type SerializedID = EntityUuid;

    fn to_serialized(&self, entity: Entity) -> Self::SerializedID {
        let id = self.entity_map
            .borrow_mut()
            .entry(entity)
            .or_insert_with(|| *uuid::Uuid::new_v4().as_bytes())
            .clone();
        id
    }

    fn from_serialized(&self, _serialized: Self::SerializedID) -> Entity {
        panic!("CustomSerializer can only be used to serialize")
    }
}

impl<'a> legion::serialize::WorldSerializer for CustomSerializer<'a> {
    type TypeId = type_uuid::Bytes;

    fn map_id(
        &self,
        type_id: ComponentTypeId,
    ) -> Result<Self::TypeId, legion::serialize::UnknownType> {
        let uuid = self.comp_types.get(&type_id).map(|x| *x.uuid());

        match uuid {
            Some(uuid) => Ok(uuid),
            None => Err(legion::serialize::UnknownType::Error),
        }
    }

    unsafe fn serialize_component<S: Serializer>(
        &self,
        ty: ComponentTypeId,
        ptr: *const u8,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> {
        if let Some(reg) = self.comp_types.get(&ty) {
            let mut result = None;
            let mut serializer = Some(serializer);

            // The safety is guaranteed due to the guarantees of the registration,
            // namely that the ComponentTypeId maps to a ComponentRegistration of
            // the correct type.
            reg.comp_serialize(ptr, &mut |serialize| {
                result.replace(erased_serde::serialize(
                    serialize,
                    serializer.take().unwrap(),
                ));
            });

            result.take().unwrap()
        } else {
            panic!("serialize_component received unserializable type {:?}", ty);
        }
    }

    unsafe fn serialize_component_slice<S: Serializer>(
        &self,
        ty: ComponentTypeId,
        storage: &dyn UnknownComponentStorage,
        archetype: ArchetypeIndex,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if let Some(reg) = self.comp_types.get(&ty) {
            let mut serializer = Some(serializer);
            let mut result = None;
            let result_ref = &mut result;
            reg.comp_serialize_slice(storage, archetype, &mut move |serializable| {
                *result_ref = Some(erased_serde::serialize(
                    serializable,
                    serializer
                        .take()
                        .expect("serialize can only be called once"),
                ));
            });
            result.unwrap()
        } else {
            panic!(
                "serialize_component_slice received unserializable type {:?}",
                ty
            );
        }
    }
}

pub struct CustomDeserializer<'a> {
    pub comp_types_uuid: &'a HashMap<type_uuid::Bytes, ComponentRegistration>,
    pub comp_types: &'a HashMap<ComponentTypeId, ComponentRegistration>,
    pub entity_map: RefCell<&'a mut HashMap<EntityUuid, Entity>>,
    pub allocator: RefCell<legion::world::Allocate>,
}

impl<'a> CustomEntitySerializer for CustomDeserializer<'a>{
    type SerializedID = EntityUuid;

    fn to_serialized(&self, _entity: Entity) -> Self::SerializedID {
        panic!("Cannot serialize with CustomDeserializer")
    }

    fn from_serialized(&self, serialized: Self::SerializedID) -> Entity {
        let mut entity_map = self.entity_map.borrow_mut();
        let entity = entity_map
            .entry(serialized)
            .or_insert_with(|| self.allocator.borrow_mut().next().unwrap());
        *entity
    }
}

impl<'r> legion::serialize::WorldDeserializer for CustomDeserializer<'r> {
    type TypeId = type_uuid::Bytes;

    fn unmap_id(
        &self,
        type_id: &Self::TypeId,
    ) -> Result<ComponentTypeId, UnknownType> {
        let uuid = self
            .comp_types_uuid
            .get(type_id)
            .map(|x| x.component_type_id());

        match uuid {
            Some(component_type_id) => Ok(component_type_id),
            None => Err(legion::serialize::UnknownType::Error),
        }
    }

    fn register_component(
        &self,
        type_id: Self::TypeId,
        layout: &mut EntityLayout,
    ) {
        self.comp_types_uuid
            .get(&type_id)
            .unwrap()
            .register_component(layout);
    }

    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        deserializer: D,
    ) -> Result<Box<[u8]>, <D as Deserializer<'de>>::Error> {
        use serde::de::Error;
        let mut erased = erased_serde::Deserializer::erase(deserializer);
        if let Some(reg) = self.comp_types.get(&type_id) {
            reg.comp_deserialize(&mut erased).map_err(D::Error::custom)
        } else {
            panic!(
                "deserialize_component received undeserializable type {:?}",
                type_id
            );
        }
    }

    fn deserialize_component_slice<'a, 'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        writer: UnknownComponentWriter<'a>,
        deserializer: D,
    ) -> Result<(), D::Error> {
        if let Some(reg) = self.comp_types.get(&type_id) {
            use serde::de::Error;
            let mut deserializer = erased_serde::Deserializer::erase(deserializer);
            reg.comp_deserialize_slice(writer, &mut deserializer)
                .map_err(D::Error::custom)
        } else {
            panic!(
                "deserialize_component_slice received undeserializable type {:?}",
                type_id
            );
        }
    }
}

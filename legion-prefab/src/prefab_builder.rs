use legion::*;
use prefab_format::{EntityUuid, ComponentTypeUuid, PrefabUuid};

use std::collections::HashMap;
use std::collections::HashSet;
use crate::{ComponentRegistration, DiffSingleResult, ComponentOverride, PrefabMeta, PrefabRef};
use crate::{CookedPrefab, CopyCloneImpl, Prefab};
use fnv::FnvHashMap;
use std::hash::BuildHasher;

pub struct EntityInfo {
    before_entity: Entity,
    after_entity: Entity,
}

impl EntityInfo {
    pub fn new(
        before_entity: Entity,
        after_entity: Entity,
    ) -> Self {
        EntityInfo {
            before_entity,
            after_entity,
        }
    }

    pub fn before_entity(&self) -> Entity {
        self.before_entity
    }

    pub fn after_entity(&self) -> Entity {
        self.after_entity
    }
}

pub struct PrefabBuilder {
    // This is the snapshot of the world when the transaction starts
    before_world: legion::world::World,

    // This is the world that a downstream caller can manipulate. We will diff the data here against
    // the before_world to produce diffs
    after_world: legion::world::World,

    // All known entities throughout the transaction
    uuid_to_entities: FnvHashMap<EntityUuid, EntityInfo>,

    parent_prefab: PrefabUuid,
}

#[derive(Debug)]
pub enum PrefabBuilderError {
    EntityDeleted,
    ComponentRemoved,
    ComponentAdded,
}

impl PrefabBuilder {
    pub fn new<S: BuildHasher>(
        prefab_uuid: PrefabUuid,
        prefab: CookedPrefab,
        universe: &Universe,
        clone_impl: &CopyCloneImpl<S>,
    ) -> Self {
        let mut before_world = universe.create_world();
        unimplemented!();
        // let mut before_result_mappings = HashMap::new();
        // before_world.clone_from(
        //     &prefab.world,
        //     clone_impl,
        //     &mut legion::world::HashMapCloneImplResult(&mut before_result_mappings),
        //     &legion::world::NoneEntityReplacePolicy,
        // );

        let mut after_world = universe.create_world();
        unimplemented!();
        // let mut after_result_mappings = HashMap::new();
        // after_world.clone_from(
        //     &prefab.world,
        //     clone_impl,
        //     &mut legion::world::HashMapCloneImplResult(&mut after_result_mappings),
        //     &legion::world::NoneEntityReplacePolicy,
        // );

        // let mut uuid_to_entities = FnvHashMap::default();
        // for (uuid, entity) in &prefab.entities {
        //     let before_entity = before_result_mappings[entity];
        //     let after_entity = after_result_mappings[entity];
        //     uuid_to_entities.insert(*uuid, EntityInfo::new(before_entity, after_entity));
        // }
        //
        // PrefabBuilder {
        //     before_world,
        //     after_world,
        //     uuid_to_entities,
        //     parent_prefab: prefab_uuid,
        // }
    }

    pub fn world(&self) -> &World {
        &self.after_world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.after_world
    }

    pub fn uuid_to_entity(
        &self,
        uuid: EntityUuid,
    ) -> Option<Entity> {
        self.uuid_to_entities.get(&uuid).map(|x| x.after_entity())
    }

    pub fn create_prefab<S: BuildHasher>(
        &mut self,
        universe: &Universe,
        registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration>,
        clone_impl: &CopyCloneImpl<S>,
    ) -> Result<Prefab, PrefabBuilderError> {
        let mut new_prefab_world = universe.create_world();

        let mut preexisting_after_entities = HashSet::new();
        for entity_info in self.uuid_to_entities.values() {
            if !self.after_world.contains(entity_info.after_entity) {
                // We do not support deleting entities in child prefabs
                return Err(PrefabBuilderError::EntityDeleted);
            }

            preexisting_after_entities.insert(entity_info.after_entity);
        }

        // Find the entities that have been added
        let mut all = Entity::query();
        for after_entity in all.iter(&self.after_world) {
            if !preexisting_after_entities.contains(&after_entity) {
                unimplemented!();
                // let new_entity = new_prefab_world.clone_from_single(
                //     &self.after_world,
                //     after_entity,
                //     clone_impl,
                //     None,
                // );
            }
        }

        let mut entity_overrides = HashMap::new();

        for (entity_uuid, entity_info) in &self.uuid_to_entities {
            let mut component_overrides = vec![];

            for (component_type, registration) in registered_components {
                let mut ron_ser = ron::ser::Serializer::new(None, true);
                let mut erased = erased_serde::Serializer::erase(&mut ron_ser);

                let result = registration.diff_single(
                    &mut erased,
                    &self.before_world,
                    Some(entity_info.before_entity()),
                    &self.after_world,
                    Some(entity_info.after_entity()),
                );

                match result {
                    DiffSingleResult::NoChange => {
                        // Do nothing
                    }
                    DiffSingleResult::Change => {
                        // Store the change
                        component_overrides.push(ComponentOverride {
                            component_type: *component_type,
                            data: ron_ser.into_output_string(),
                        })
                    }
                    DiffSingleResult::Add => {
                        // Fail, a component was added. This is not supported
                        return Err(PrefabBuilderError::ComponentAdded);
                    }
                    DiffSingleResult::Remove => {
                        // Fail, a component was deleted. This is not supported
                        return Err(PrefabBuilderError::ComponentRemoved);
                    }
                }
            }

            if !component_overrides.is_empty() {
                entity_overrides.insert(*entity_uuid, component_overrides);
            }
        }

        let prefab_ref = PrefabRef {
            overrides: entity_overrides,
        };

        let mut prefab_refs = HashMap::new();
        prefab_refs.insert(self.parent_prefab, prefab_ref);

        let prefab_meta = PrefabMeta {
            id: *uuid::Uuid::new_v4().as_bytes(),
            prefab_refs,
        };

        Ok(Prefab {
            world: new_prefab_world,
            prefab_meta,
        })
    }
}

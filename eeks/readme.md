# Extendable Entity Komponent System (Extendable EKS (EEKS))
EEKS is an entity component system of my own design. 
It features rust code hot reloading, lua scripting, and integration with my rendering system. 

Ekstensions is an extension to EKS with a hot-reloadable ECS. 
An extension is a crate which complies to a shared library. 
This library exports the `dependecies`, `systems`, and `load` functions. 

The `dependecies` function describes which `load` functions are to be called before this extension's `load` function. 
It is useful if an extension is to override another extension or creates a resource which depends on the resource of another extension. 

The `systems` function is (meant to 
be but is not currently) called after the `load` functions. 
It specifies all systems provided by this extension and their run order. 
In the future we should allow for the conditional enabling of systems (for compatibility and patching), but this is not currently implemented. 

The `load` function declares components and resources to the world. 
All `load` functions are run in one thread, so it is advised to avoid computationally intensive work here. 
Work-heavy initialization should happen in systems in the `init` group because these systems may be run in parallel. 

When an extension is included as a dependency, the `no_export` feature must be enabled. 
This prevents the linker from becoming confused by multiple exported `dependecies`, `systems`, and `load` symbols. 

## Environment 
EEKS pulls some settings from environment variables. 

| Variable | Default | Function |
| - | - | - |
| EEKS_SCCACHE | true | Tries to compile crate extensions with sccache wrapper. Will disable itself and print an error if sccache is not accessible. |
| EEKS_DEEP_CHECKING | true | Looks for `.d` files in crate extension output directories. Uses the content to more accurately test for most recent modification. |
| EEKS_BATCHED | true | If there are multiple dirty crate extensions that are part of the main program's workspace, this will batch their compilation by building the entire workspace. |

## To Do
Serialization in reloading. 
We must decide how to serialize component storages. 

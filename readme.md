# Kkraft
Kkraft is like Minecraft but it sucks.


# Dependencies
Mlua
Gilrs


ECS design, Rust loves data ownership.
Don't let everything have access to everything.
The borrowing will tear you apart.
OO will cause pain.
So, separate data from functions.

Use vec and indices, leave arena and rc for experts.

Generational indices are indices with a generation.
Alloc => 0
Alloc => 1
Dealloc 0
Alloc => 0


ECS:
System: Entity, physics, input, UI, (chunk meshing)?, (chunk blocktick?)
Movement component takes input and gives physics position component



# Implementation

Stream the world in chunks from the player location.

Stitch all textures into a sprite sheet, then bind the sprite sheet.

Opaque objects are grouped together as a single mesh.
This causes issues with transparency.

Only render sides exposed to air.
Chunks record an overlap with the next chunk to know if the edge bits should be sided.
Try to update the VBO instead of regenerating the mesh when a block is changed.

SSAO is good

  ## Lua Integration
  https://snoozetime.github.io/2019/01/29/scripting-gameengine.html

  ## Rendering
  Rendering could use rasterization or ray tracing.
  
  WGPU is built to be thread safe

  How can I render things?
  Can I use a gpu?

   ### Ray-Tracing
   Needs a fast ray-box intersection algorithm.

   ### Rasterization
   Idk how to do this

  ## Operation
  Operations should be divided into different threads.
  Sound, Render, Chunks, Entities

  Threads should communicate using messaging 
  (https://doc.rust-lang.org/book/ch16-02-message-passing.html)

   ### Blocks
   Blocks are defined by a data file and an event file.
   
   A data file is a json or toml file containing various variables.
   Things like texture and hardness are defined here.

   An event file is a lua script containing functions for various events the block may experience.
   If not function is defined for a given event then a default behaviour is used.
   Things like ticking behaviour, breaking behaviour, and click events are stored here.
   See (https://github.com/Technici4n/rust-voxel-game/blob/master/async.md) to know why this might be a bad idea.

  ## Chunking
  World data should be stored in chunks, a kind of paging.
  A chunk stores blocks in an interval tree.
  (https://0fps.net/2012/01/14/an-analysis-of-minecraft-like-engines/)
  (https://github.com/mikolalysenko/NodeMinecraftThing/blob/master/client/voxels.js)

  Multithreaded chunking:
  Would cause delay if tick events affect other chunks.

   ### Regions
   Region Chunking:
   Word divided into regions, one per core

   ### Chunk Data
   ToTick, Entities, (lights?)



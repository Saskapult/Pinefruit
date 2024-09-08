# Pinefruit ![immature pinecone](resources/textures/pinefruit.png)
![image showcasing voxel terrain](example_1.png) 

There comes a time in a programmer's life where they might feel the call. 
The beckoning wears at them, and in their heart they know what must be done. 
In waking and in sleep, the challenge wears away at their mind. 
It is now their turn to make a Minecraft clone. 

Pinefruit a thing that I made to learn more about rendering and game engine architecture. 
Its visual design is based around Minecraft because Minecraft looks cool. 
If n*tch can do it so can I. 

## Controls 
- Left click with mouse to lock cursor, drag to look around, right click to release cursor 
- WASD to move 
- E and Q to place or remove voxels respectively 
- T to place light 

## Building 
This project utilizes features that require a nightly version of the rust compiler.
At the time of writing, I am using `rustc 1.79.0-nightly`. 

Rust-based extensions that are not bundled with the core application may take a very long time to compile. 

## Features 
- Multithreaded terrain generation 
- GPU-based voxel ray tracing 
- Data-driven rendering system 
- Hot reloading of code (for both lua and rust!)
- Lua scripting integration 
- Console command system 

## Crates 
- EKS - Entity component system 
- EEKS - Dynamic extension loading for EKS 
- Krender - Rendering helper 
- Oktree - Octree data structure based on [Efficient Sparse Voxel Octrees](https://research.nvidia.com/sites/default/files/pubs/2010-02_Efficient-Sparse-Voxel/laine2010tr1_paper.pdf) 

# Krender
While developing Kkraft I experimented with a lot of different rendering techniques. 
Eventually I became frustrated by the need to recompile my program with every change to the shaders. 
This prompted me to add shader hot-reloading, which was great for a time. 
But then I realized I could make it a lot better. 

Krender handles shader reloading, the supply of input data, and automatic model batching. 

Note: This project is made by me for me, and a lot of things might be broken at any given time. 

One feature of Krender which I haven't encountered in other rendering helpers is the ability to pull instance data from an external container. 
Krender does this through a trait. 
I haven't used it with anything other than EKS, my entity component system. 
If you want an example of the integration, please look at Kkraft. 

## To Do
- At the time of writing, static batches are not functional 
- Input inserts should all return keys 
- Persistent buffers/textures are not actually persisted across rebinds
- Readable buffers/textures are not actually readable

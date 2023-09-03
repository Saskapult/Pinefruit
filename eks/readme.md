# Entity Komponent System
I needed an ECS crate with *a very specific set of skills*. 
I also didn't care about having competitive performance, only "good enough" performance. 
I also didn't understand how ECSs worked internally and wanted to learn more. 
Also I thought to myself "hey I could put this on my resume!" 
Thus, EKS was born.

EKS is an ECS with inspiration drawn from EnTT, Shipyard, Sparsey, and hecs. 
It uses a sparse set to store everything. I don't know what else to say. 

As mentioned earlier, EKS will not have competitive performance. 
I don't need that much power for my hobby projects. 
That being said, it should still be efficient enough for most purposes. 
In the future I might use [ecs_bench_suite](https://github.com/rust-gamedev/ecs_bench_suite) to see if something is obviously wrong.

EKS is not safe!
EKS is intended to live in a webassembly component or dll file where it will interact with unknown data and call unknown functions.
This might come back to haunt me but at this moment I don't see an alternative.

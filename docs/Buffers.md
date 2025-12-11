# Buffers
I prefer to only work on the code but documenting is as important. So here we are now. 

Particularely I want to explain how the texture sources work since I have some comments asking `\\ ping or pong?`. 

## Ping Pong Buffer
There are two buffers. I called them source 1 and source 2 in the code. Both buffers have the same size. A source is accepting new data, aka `write`, while the other source's data is being processed, aka `read`. When this process is done they switch their roles. In this way the data flow is smooth and we can prevent stalling the system and corrupting data. 

To make this ping-pong buffer I needed
- 2 texture sources
    - source 1 
    - source 2
- 2 compute
    - compute from source 1 and write to source 2
    - compute from source 2 and write to source 1
- 2 rendering
    - render from source 1 when compute wrote to it
    - render from source 2 when compute wrote to it

## Uniform
To store time I used a uniform buffer (which I haven't used it the way I actually wanted to.. because it made the simulation to go sooo slooooow that I was not happy about it so I hard coded a `dt` for now. I might make the colors time dependent for some hallucination effect lmao but then I have add the binding to the textures and not compute... so let's see.).

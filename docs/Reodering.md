# Reordering/Making a Mini Engine

The whole idea of creating the render graph came to me because I wanna build an UI and different post processing passes. Also building something like and engine that I can use for my future works sounds pretty good. So building an **Abstraction layer** that manages resources could make my life later easier.

So at the end I want to be able to tell my program that "I have these nodes... do it! make no mistake" instead of "Ok please do this first and then do that... when finished please do this."

As of writing this document I am reading more about Directed Acyclic Graph or DAG.

My mini-engine can be qualified as a Render Graph only when it does the following jobs automatically:

1. DAG: Execution order based on dependencies and not insertion orders
2. Aliasing: Graph must calculate the lifespan of every resource to safely overlap memory
3. Automatic Synchronization: GPU pipeline needs a Transition in between to ensure the write is finished before the read starts

**At the writing of this document my Render Graph cannot do any of the above! I am still working on it.**

## Layout

For now the layout I decided to go with is as following

- reaction_diffusion_wgpu/
  - shaders/
    - brush_compute.wgsl
    - rd_compute.wgsl
    - rd_display.wgsl
  - src/
    - gpu_resource.rs
    - rd_system.rs
    - shader_watcher.rs
    - state.rs
    - render_graph/
      - graph.rs
      - mode.rs
      - resource_registry.rs
      - mode.rs
    - nodes/
      - rd_node.rs
      - mode.rs

Of course, as the project gets bigger and bigger this structure will be changed. I am also not very sure about my naming conventions for the functions and that is something I have to put more and more time into it. I might change the name of the functions while writing this document if it feels like the name doesn't work as good.

We can start from

## Render Graph

### Resource Registry

The resource registry has as of now two structs and one implementations.
The `TextureResource` is the strcuct that groups the texture and the views together. Pretty straightforward. The `ResourceRegistry` also has two entries, textures and views. I used String `HashMap` for look up so I just use the name instead of actually passing references around. I am not sure how good it works performance-wise since it is a string hash... it could be not as fast as it should be. But it works for now. The implementation has the following functions:

- new : I can actually being a registry.
- get_texture: return the texture with this name.
- get_view : If there is a texture entry with this name, return its view. Otherwise, check if there is already a registered standalone view.
- color_texture_creator : If this texture does not yet exist, create it now with the correct size and format for a render target.
- storage_texture_creator: If this texture does not yet exist, create it with the correct size and format for a render target. (But not right now like color_texture_creator)
- set_view: get the view only without the tecture with the correct name

### Node

The node trait is a contract! Everything that claims to be a Node must have (some are optional actually) the following methods

- MUST have
  - name : is actually just for debugging
  - execute : the node must be executed every frame
  - as_any : `Any` allows runtime type checks.
- optional
  - prepare : prepares the BG for example
  - called_on_hotreload : when we change anything in the shaders it will rebuild the pipelines

Because execute is per frame I thought I would make sense to put per_frame_parameters in this script too.

### Graph

Our graph has one struct with registry and nodes. The implementations has the following methods:

- new : to construct the RenderGraph
- add_node : When a node is constructed we can add the node
- prepare : for every node the BG etc. must be prepared.
- execute : this method executes the node at every frame
- notify_on_hotreload_graph : it notifies the script which then calls the hot reload method that rebuilds the pipelines.
- get_node_mut: Needed for downcasting to access methods from the nodes.

## Nodes

**I want to separate the Brush Node from RD Node later down the line. Right now both are in one node**
Right now the I made it very simple as in Sim -> Display -> Sim -> Display and so on.

### RD Node

There are two structs, namely, `ReactionDiffusionSimulationNode` that handles the compute and `ReactionDiffusionDisplayNode` that handles the rendering.

Sim node has a simulation logic and a reset logic. While `ReactionDiffusionDisplayNode` has the BGL, pipeline and the sampler. They are all `Option<>` because when creating the shared logic the don't exist.

The `create_rd_shared_nodes` function is meant to construct both structs and the shared logic of course.

#### Making ReactionDiffusionSimulationNode

Now it is time to implement the contracts we made for nodes (`trait`) for Sim node. The prepare function here uses the the storage creator method from registry. We make two texture storage `rd ping` and `rd pong`. In the prepare function we also get the textures and use the `write_pattern_to_starting_space` helper function to restart the simulation when we want. Right now this execute method also has Brush logic and scope for uploading the parameters.

#### Making ReactionDiffusionDisplayNode

When the calculation is finished it is time time to use the results to render. We can prepare the rendering by loaidng the rendering code.
**Just a reminder for myself: I couldn't use `include_str!` because it was a micro at compile time and so the hot reload was not doable since the program needs to find the shader at runtime.**
So I just create the module, BGL, pipeline and its layout. The Bindings are right now as following

- binging 0 : RD Texture
- binding 1 : RD Sampler

The hot reload rebuilds the BG and the pipelines for rendering everytime anything in the display shaders is changed.

## Future plans?

I feel like making the BG every frame is a problem. Well not a problem but it must lead to some overhead that is not really necessary. Even though it is working but I should be able to optimize it.

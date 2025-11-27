# Reaction Diffusion

My first idea for this project wasn't reaction diffusion in the beginning. I was more leaning towards N-Body problem or even fluid simulation. But because, my PhD project is just physics, I decided to get away from physics a bit but stay mathematical. Because I was interested in doing some `compute` pipelines in my project. So I decided to go with fractals. In the search for fractals that I could use for this project I saw some cool patterns which turned out to be a **Reaction Diffusion System**.

> a reaction diffusion refers to a mathematical model that describes how two or more chemicals react with each other and diffuse through a medium over time.

The raction diffusion can be mathematically described as:

```math
\frac{\partial u}{\partial t} = D_u \Delta u + f_u(u, v)\\

\frac{\partial v}{\partial t} = D_v \Delta v + f_v(u, v)\\
```

where $\Delta$ is the Laplacian and since we are in 2D:

```math
\Delta = \nabla^2 = \frac{\partial^2 }{\partial x^2} + \frac{\partial^2 }{\partial y^2}
```

The time to proceed and do the numerical calculation will come soon and I will expand the explanation.

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

## Laplace Operator

When we want to calculate the Laplacian numerically, we can use finite elements. In two dimensions the Laplacian can be written as the following operator:

```math 
L_{2D} = \begin{pmatrix} 
0 & 1 & 0 \\
1 & -4 & 1 \\
0 & 1 & 0
\end{pmatrix}
```

which corresponds to 

```math
\nabla^2 f(x, y) \approx f(x, y - 1) + f(x - 1, y) - 4 f(x, y) + f(x + 1, y) + f(x, y + 1)
```

In the numeric literature, we find the same equation but instead of $1$ we have an infinitesimal element of $h$. The whole RHS of the equation is also multiplide with $1/h^2$. Here we simply say $h = 1$ which is the grid spacing. This is coded in compute shader: 

```wgsl 
fn laplacian(texture: texture_2d<f32>, x_y: vec2<i32>) -> vec2<f32> {
    let center = read_u_v(texture, x_y);
    let up = read_u_v(texture, x_y + vec2<i32>(0, -1));
    let down = read_u_v(texture, x_y + vec2<i32>(0, 1));
    let left = read_u_v(texture, x_y + vec2<i32>(-1, 0));
    let right = read_u_v(texture, x_y + vec2<i32>(1, 0));

    let laplace = (up + down + left + right) - 4.0 * center;
    return laplace;
}
```

# Post Processing

## Fake 3D

The input textures are the two elements (chemicals).
Let's call $u_{i,j}$ texture in red channel at pixel $(i,j)$, while the green channel has texture $v_{i,j}$ at pixel $(i,j)$.

Then we define a hight field $h_{i,j} = u_{i,j} - v_{i,j}$. (It is needed for the surface normals later on)

Then, the neighbor coordinates for a point at $(i, j)$ are

- $l = (i-1, j)$
- $r = (i+1, j)$
- $u = (i, j-1)$
- $d = (i, j+1)$

We can now calculate the partial derivatives:

```math
\frac{\partial h}{\partial x} = \frac{Hright - Hleft}{2}
```

where

```math
Hleft = u_{i-1, j} - v_{i-1, j}
```

and

```math
Hright = u_{i+1, j} - v_{i+1, j}
```

We do the same for the y-axis, then we have $\nabla h = (\frac{\partial h}{\partial x} , \frac{\partial h}{\partial y} )$. If we had truly a 3D surface

To give a feeling that our simulation is 3D, we add a third axis z prependicular to the x-y plane. This 3d world can constructed as

```math
S = \{(x, y, z) \in R³ | z = h(x = u, y = v)\}
```

it is a bumpy sheet. We can now define an implicit surface which is a surface defined by an equation $F(x, y, z) = 0$. Using the equation for $z$ we have $F(x, y, z) = z - h(x, y) = 0$, yielding for the gradient:

```math
\nabla F = (-\partial_x h, -\partial_y h, 1)
```

I do multiply the x-y plane contribution with a slope scale in order to exaggerate the tilt. It is not mathematical. It is just for cool visuals.

Now, I can normalize the gradient (which is the surface normal here): $n = \nabla F / ||\nabla F||$.

## Lambert

Now to actually give the 3D feeling, we choose a position for a fake light at $L$, which the direction $||L||$.

Now we define `diffuse` in Lambert term: $d = n \cdot L = ||n||\,||L|| \cos(\theta)$ (Lambertian reflection) meaning:

- If light and the surface normal point the same way, diffuse term is at its maximum.
- If light and the surface normal are perpendicular, diffuse term is at its minimum.

We define shade `ambient + (1.0 - ambient) * diffuse`. In this way, `shade = ambient` when `diffuse = 0` which renders something like a shadow. And when we have full light, i.e., `diffuse = 1` the shade is 1, i.e., full brightness.

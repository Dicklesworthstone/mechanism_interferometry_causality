# Partial and Fractional Designs

For observed corners `D`, flatness is a main-effects-only model with a function-valued response:

\[
h_D(x)=M_D\beta(x).
\]

The testable flatness space is `ker(M_D^T)`. This imports classical design-of-experiments theory without changing its algebra.

## Design audit

The program computes:

1. rank of `M_D`;
2. dimension and basis of the lack-of-fit space;
3. all complete square-face contrasts;
4. the span of those square contrasts;
5. non-face contrasts needed to complete the lack-of-fit basis;
6. aliases between main effects and interaction columns;
7. the generalized word-length pattern for regular fractions when available.

## Six-corner counterexample

The 3-cube with `000` and `111` removed contains no complete square face. Nevertheless, six observations minus rank four leaves two flatness restrictions. One is

```text
h110 - h100 - h011 + h001 = 0.
```

A square-only implementation would incorrectly declare that no flatness content is testable.

## Engineered-system recommendation

For many flags, use a resolution-V fraction when pairwise curvature is the scientific target. It keeps two-factor interactions unaliased from main effects. Confirmatory additions can then target faces selected by the first-stage fraction.

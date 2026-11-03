# MVP 1 option valuation rules

**Design status:** Accepted pricing-method scope for the synthetic MVP.

**Implementation status:** The Rust engine includes the synthetic quote, Black–Scholes, and CRR valuation paths with controlled checks. This does not claim real-market pricing fidelity or complete MVP acceptance.

## Pricing modes

Each option instrument uses one configured valuation mode for its run:

- `quote`: the instrument's close quote is the portfolio valuation mark.
- `theoretical`: the configured option pricer produces the portfolio valuation mark.

The European option uses Black–Scholes. The American option uses a recombining Cox–Ross–Rubinstein binomial tree. Both use the same price date, underlying close mark, strike, expiry, multiplier, annualized volatility, continuously compounded annual risk-free rate, and configured mode. The baseline has zero dividends; dividend cashflows and Greeks are out of scope.

The theoretical mark is used for NAV and maintenance calculations only. Fills always use a fixture open execution quote; absence of that quote is an execution failure. A model price is never an assumed executable price. The quote and theoretical mark, all inputs, method, step count, and implementation version are stored with run evidence.

## Time and input availability

For valuation time `t` and expiry `T`, use ACT/365F: elapsed UTC seconds divided by `31,536,000`. At or after expiry, return cash intrinsic value for a European contract and the corresponding immediate exercise value for an American pricing tree. Exercise/assignment still requires a distinct fixture event; theoretical value does not create a trade or physical delivery.

At each valuation time, select the latest `option_pricing_input` observation for the option whose `available_at` is no later than that time. The observation carries a continuously compounded annual `risk_free_rate` and `annualized_volatility`, with source/version and event/availability times. Baseline values are rate `0` and volatility `0.2`. Volatility must be positive. Fixed algorithm choices (pricer, ACT/365F, and CRR steps) are declared in run configuration; time-varying inputs remain point-in-time data. If a theoretical mode lacks a valid underlying mark, expiry, rate, or volatility, fail the run with a pricing-input error. Do not silently fall back to quotes.

Restrict MVP 1 parameters to annualized rate in `[-1, 1]` and annualized volatility in `(0, 5]`; these broad synthetic bounds protect arithmetic inputs and do not imply market calibration. Reject an invalid rate, volatility, strike, time, negative underlying price, non-finite intermediate, invalid tree probability, or arithmetic overflow with a typed pricing error. Intrinsic value handles zero time directly without a logarithm or division by volatility. At `S=0` and positive time, return the Black–Scholes zero-underlying limit (zero call, discounted strike put) rather than evaluating `ln(S/K)`.

## Black–Scholes value for European options

Let `S > 0` be the underlying mark, `K > 0` the strike, `r` the continuous annual risk-free rate, `σ > 0` annualized volatility, and `τ > 0` ACT/365F years to expiry. Let `N(x)` be the standard normal cumulative distribution function.

`d1 = (ln(S/K) + (r + σ²/2)τ) / (σ√τ)`

`d2 = d1 - σ√τ`

`call = S N(d1) - K exp(-rτ) N(d2)`

`put = K exp(-rτ) N(-d2) - S N(-d1)`

The zero-dividend assumption is part of the input scope. Contract multiplier scales the resulting per-unit option value only in portfolio valuation. The price returned for the option must be finite and nonnegative.

## Cox–Ross–Rubinstein value for American options

Declare `N` in the American option's theoretical valuation configuration. The accepted baseline uses `N=256`; valid values are bounded from 2 through 4096. Let `Δt=τ/N`, `u=exp(σ√Δt)`, `d=1/u`, and `p=(exp(rΔt)-d)/(u-d)`. Reject the inputs if `p` falls outside `[0,1]` or any tree intermediate is not finite.

At expiry nodes, value calls by `max(S_node-K, 0)` and puts by `max(K-S_node, 0)`. Move backward through the tree, discounting the probability-weighted node values by `exp(-rΔt)` and taking the greater of continuation value and immediate intrinsic value at every American node. Return the root value. The method is an American exercise valuation model; it does not determine a simulator holder's exercise or counterparty assignment.

## Numerical standard library

Use stable Rust `f64` logarithm, exponential, and square-root operations for theoretical marks only; convert each resulting mark once to a decimal rounded half-even to 12 fractional places. Keep portfolio cash and fills in the decimal ledger; record the rounded theoretical mark separately from the fill price.

Implement `N(x)=0.5×(1+erf(x/√2))` using Abramowitz–Stegun formula 7.1.26: `erf(x)=sign(x)×(1−P(t)exp(−x²))`, `t=1/(1+0.3275911×|x|)`, `P(t)=((((1.061405429t−1.453152027)t+1.421413741)t−0.284496736)t+0.254829592)t`. The approximation's maximum absolute error is `1.5×10⁻⁷`. Do not call Rust's nightly-only `f64::erf`; the stable standard library does not make that method available.

Acceptance references at `S=100`, `K=100`, `r=0.05`, `σ=0.2`, and `τ=1` year are European Black–Scholes call `10.45058357` and put `5.57352602` (absolute tolerance `0.00005`). With 256 CRR steps, European call `10.44277600`, European put `5.56571845`, American call `10.44277600`, and American put `6.08725833` (absolute tolerance `0.00000002` for tree values). Additional tests cover zero-time intrinsic limits, put-call parity at zero dividends, spot monotonicity, volatility monotonicity, probability bounds, deterministic repeated inputs on the same supported platform, and rejection of missing/future inputs. These are future Phase B checks, not code tests run in Phase A.

Option-price outputs are analytical approximations, not broker quotes, return forecasts, or guaranteed exit proceeds. Market quotes remain the fixture execution prices.

## References

- Black and Scholes, *The Pricing of Options and Corporate Liabilities* (1973), [Journal of Political Economy](https://www.journals.uchicago.edu/doi/pdf/10.1086/260062).
- Cox, Ross, and Rubinstein, *Option Pricing: A Simplified Approach* (1979), [working paper](https://bpb-us-w2.wpmucdn.com/u.osu.edu/dist/7/36891/files/2017/07/CRR79-1yy8av8.pdf).
- Abramowitz and Stegun, *Handbook of Mathematical Functions*, NBS Applied Mathematics Series 55 (1964), [NIST record](https://www.nist.gov/mathematics-statistics/handbook-mathematical-functions-abramowitz-and-stegun).
- Rust [`f64` documentation](https://doc.rust-lang.org/std/primitive.f64.html) for the host's standard floating-point methods and their precision guarantees.

# Public causal dataset gauntlet

## Purpose

This is a benchmark portfolio for causal mechanism tomography, not a list of
interesting correlations. Each dataset is included because it exercises a
different implication of causality: intervention response, autonomous mechanism
reuse, propagation, composition, effect heterogeneity, or mandatory abstention.

Dataset access, licenses, schemas, and published ground truth must be verified
and frozen in an ingestion receipt before a run. A published graph can be wrong;
the benchmark records whether its truth comes from construction, randomized
actuation, physical topology, expert consensus, or a disputed observational
interpretation.

## Priority portfolio

| Priority | Dataset | Scale and regimes | Causal capability exercised | Ground-truth authority | First falsifier |
|---|---|---|---|---|---|
| P0 | [Causal Chambers](https://www.causalchamber.ai/) | Large real sensor traces from computer-controlled light and wind systems; interventions and open hardware | Unknown-target recovery, repeated tilts, conditional invariance, time response, held-out interventions | Physical actuation plus experimentally validated chamber models; data and hardware are public | Hide intervention metadata: the system must lose authority or abstain, not reproduce the labeled answer from leakage |
| P0 | MIC autonomous-chain gauntlet | Exact synthetic multi-environment laws with single and combined soft interventions | Complete end-to-end oracle for locality, normalization, universal deletion, graph assembly, flatness, state expansion | Structural equations and exact enumeration | One wrong edge, one self-normalized product, or one pass under a violated contract kills promotion |
| P0 | [DREAM4 in-silico networks](https://www.bioconductor.org/packages/release/data/experiment/html/DREAM4.html) | 10- and 100-node networks with perturbations and known solutions | Graph recovery, single/dual perturbations, held-out regime prediction, active design | Simulator graph and intervention generator | Performance must collapse honestly under hidden targets or omitted variables rather than invent certainty |
| P1 | [CausalRivers](https://causalrivers.github.io/) | 666 eastern-Germany and 495 Bavaria gauges, 15-minute data over 2019–2023, plus a flood shift | Large-scale propagation, ancestor partial order, change points, missing paths, confounding stress | River-network/topology-derived graphs; observational discharge remains weather-confounded | Random time shifts and sensor-latency perturbations must not create confident reverse flow |
| P1 | [European Spallation Source industrial benchmark](https://proceedings.mlr.press/v236/mogensen24a.html) | Multivariate subsystem time series with operator inputs | Engineered-system graph recovery, control actions, feedback and latency handling | Expert-constructed graph from a real industrial system | With operator inputs hidden, edges dependent on actuation metadata must downgrade |
| P1 | [CausalBench single-cell perturbations](https://github.com/causalbench/causalbench) | Two open CRISPR single-cell experiments with more than 200,000 interventional observations | Unknown intervention targets, sparse mechanism shifts, high-dimensional localization, intervention-response prediction | Interventions are real; gene-network “truth” is partial and metric-dependent | Score on held-out intervention response as well as disputed graph edges; do not call biological databases exact truth |
| P1 | [LINCS L1000](https://commonfund.nih.gov/lincs) | Chemical and genetic perturbations across cell types, doses, and times; hundreds of thousands of signatures | Repeated tilts, mechanism vocabulary induction, dose/time transport, cross-cell-context reuse | Perturbation delivery labels are strong; full downstream graph is not known | Leave out an entire perturbagen-context combination and require calibrated prediction plus raw normalization diagnostics |
| P1 | Norman Perturb-seq | Large single- and double-guide expression experiment already under repository study | Factorial composition, curvature, hidden-state recovery, selection/unit discipline | Guide identities are known; expression-state graph and selection mechanism are not | Gemgroup-held-out field stability, four-corner occupancy, raw `Z`, and selection abstention are mandatory |
| P1 | [Criteo uplift dataset](https://ailab.criteo.com/criteo-uplift-prediction-dataset/) | 13.9 million rows in the corrected release from randomized advertising incrementality tests | Treatment-effect heterogeneity, randomized anchor relevance, scale and calibration | Randomized treatment assignment; anonymized features and non-uniform subsampling limit mechanism claims | Use the corrected release; advertiser or sampling leakage must be detected by environment holdouts |
| P2 | [UCI hydraulic test rig](https://archive.ics.uci.edu/dataset/447) | 2,205 repeated 60-second cycles and 43,680 sensor features; four components varied over fault severities | Multi-actuator response signatures, fault localization, feedback/time dynamics, fat-intervention abstention | Controlled physical test rig; component conditions are recorded | The scout must distinguish “which component was altered” from “which sensor is causally upstream” |
| P2 | [UCI dynamic gas mixtures](https://archive.ics.uci.edu/dataset/322/gas%2Bsensor%2Barray%2Bunder%2Bdynamic%2Bgas%2Bmixtures) | Dynamic concentration mixtures and a chemical sensor array | Known actuator-to-sensor direction, mixture composition, response delay, sensor drift | Gas concentration program is an input; sensor responses are outcomes | Time reversal and concentration-label shuffle must block direction; mixture curvature must not be hidden by normalization |
| P2 | [NASA C-MAPSS](https://data.nasa.gov/dataset/cmapss-jet-engine-simulated-data) | Hundreds of run-to-failure engine trajectories, six operating conditions, fan/HPC faults | Longitudinal state, degradation propagation, domain shift, operating-condition adjustment | High-fidelity simulator with recorded fault modes; not a real-engine causal truth | Report simulated authority; an operating condition mistaken for a fault is a failure |
| P2 | [OpenNeuro auditory oddball EEG](https://openneuro.org/datasets/ds003061/versions/1.1.1) | Randomized/jittered auditory events, repeated sessions, multichannel EEG, participant units | Exogenous impulse response, latency-aware ancestry, representation sufficiency | Stimulus timing and type are controlled; brain-network edges are not ground truth | Future-stimulus and pre-stimulus placebos must be null; participant, not sample, is the uncertainty unit |
| P2 | [CauseMe](https://causeme.uv.es/) | Synthetic and real multivariate time-series challenges with provided high-confidence structures | Cross-method time-series benchmark, nonlinear dynamics, hidden confounding and feedback stress | Varies by challenge and must be recorded per dataset | Never aggregate scores across truth-authority classes without stratification |
| P3 | [Tübingen cause-effect pairs](https://webdav.tuebingen.mpg.de/causality/) | 83 heterogeneous two-variable pairs in the current curated archive | Static-direction baseline, algorithmic independence/ANM comparison, forced-abstention calibration | Curated direction labels, but no environment/intervention information; archive license is currently listed as TBD by the Zenodo mirror | Mirrored constitutive and deterministic pairs must expose where passive two-variable direction is not identified |
| P3 | [NOAA CO-OPS water levels](https://api.tidesandcurrents.noaa.gov/api/prod/) | Six-minute/hourly observed water levels, meteorology, and tide products over many stations | Astronomical/environment response, station transfer, storm residuals | Observations are official; harmonic predictions are derived from past water levels and can leak the target | Independently compute or withhold the forcing phase; do not treat a fitted tide prediction as an external intervention |
| P3 | [USGS continuous water values](https://api.waterdata.usgs.gov/) | National streamflow/gauge network with high-frequency historical measurements | Propagation, upstream/downstream partial order, event holdouts | Sensor data and hydrologic topology are strong; rainfall/common-catchment confounding remains | Unknown routing, dams, clock offsets, and common rainfall require ancestor-only or abstained outputs |
| P3 | [NOAA ISD](https://www.ncei.noaa.gov/products/land-based-station/integrated-surface-database) | More than 35,000 stations and hourly observations dating to 1901 | Cross-site environment shifts, measurement changes, physical sanity relations | Official observations/metadata; geography is not randomized | Use raw station pressure for elevation physics; sea-level-adjusted pressure and spatial confounding are explicit traps |
| P3 | [NYC TLC trip records](https://www.nyc.gov/site/tlc/about/raw-data.page) joined to official weather | Millions of trips with time and location | Exogenous-shock field stress, heterogeneity, spatial spillovers, policy changes | Trip/weather records are official; causal exclusion and selection are assumptions | Future-weather, neighboring-city, holiday, and reporting-regime placebos must accompany any arrow |
| P3 | [EIA hourly electric-system data](https://www.eia.gov/opendata/index.php/api) joined to weather | Hourly demand, forecasts, generation, and interchange by balancing authority | Weather/load response, network propagation, forecast-vs-realized mechanisms | Official operations data; API key required and grid feedback is real | Weather may affect both load and generation; report coupled mechanisms instead of forcing one edge |

## Why these datasets fit together

### Mechanism recovery

Causal Chambers, DREAM4, CausalBench, LINCS, and Norman contain deliberate
changes. They ask whether the system can recover a reusable mechanism vocabulary
and predict unseen combinations, not merely rediscover treatment labels.

### Propagation and partial order

CausalRivers, the ESS subsystem, hydraulic rigs, EEG events, USGS gauges, and
engine trajectories expose time and network structure. They test whether the
system says `ancestor` when that is all timing establishes, and whether it
abstains under feedback, shared forcing, or uncertain latency.

### Composition and interference

Norman double perturbations, DREAM4 dual knockouts, gas mixtures, factorial
Causal Chamber experiments, and dose/context combinations in LINCS are where
mechanism interferometry should be most distinctive. The primary prediction is
a held-out law, not a scalar interaction score. Every compositional result must
include raw normalization and common-support diagnostics.

### Effect identification rather than graph discovery

Criteo's randomized trials and future public RCT/RD datasets test the system's
ability to select and execute a valid identification contract. They should not
be scored as whole-DAG benchmarks. This distinction lets the product answer
useful causal questions even when global structure remains unidentified.

### Mandatory abstention

Tübingen pairs, weather joins, and observational river data are valuable partly
because their assumptions are incomplete. A system that produces an arrow on
every pair fails this portfolio even if its average orientation accuracy looks
good.

## Benchmark protocol

Every dataset adapter must emit a frozen receipt containing:

- source URL, retrieval timestamp, version, byte hash, and license/terms;
- raw-to-analysis transformation hash;
- unit, environment, action, state, time, and selection roles;
- why the ground truth is believed and its authority class;
- discovery and confirmation unit hashes;
- every deterministic seed;
- preregistered causal questions and allowed answer vocabulary;
- negative controls, leakage checks, and strategy-specific falsifiers;
- expected resource budget and a bounded sample for continuous integration.

The evaluation vector is lexicographic:

1. minimize wrong directed claims on identified and adversarial cases;
2. require abstention on observationally equivalent or contract-violating cases;
3. maximize correct oriented families and ancestors;
4. maximize held-out interventional/combination-law prediction;
5. minimize abstention only after the first four constraints;
6. minimize compute and the cost of the recommended next experiment.

Aggregate leaderboards must be stratified by ground-truth authority and causal
task. A graph edge backed by simulator construction is not commensurate with an
edge copied from a disputed biological consensus network.

## Recommended execution order

1. Ship the exact autonomous-chain and adversarial conformance worlds.
2. Run Causal Chambers end to end; it is the cleanest bridge from exact fixtures
   to real, controlled physics.
3. Run DREAM4 for scalable known-graph iteration and held-out combinations.
4. Run one large biological intervention corpus (CausalBench or LINCS) and keep
   Norman as the explicit factorial composition pilot.
5. Add CausalRivers and the ESS benchmark for real network dynamics.
6. Add Criteo for randomized effect-identification scale.
7. Only then use field joins such as weather, taxis, load, and public policy,
   where identification assumptions dominate the numerical work.

This ordering gives every new algorithm a cheap exact refuter, a controlled-real
test, a scalable synthetic test, and a messy field test. Passing only one layer
does not establish general causal discovery.

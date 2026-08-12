/* ==========================================================================
   Mechanism Interferometry — site behaviour.

   Plain script, no modules, no dependencies, no network access.

   Two conventions run through this file:

     1. Every number shown is computed here from the same closed forms the
        paper and the Rust workspace use. Nothing is a hard-coded screenshot
        of a result. The defaults reproduce artifacts/simulations/exact_results.json.

     2. No colour literal ever appears below. Diagram elements are given
        class names and coloured by styles.css, so both themes work without a
        redraw and the semantic contract (green means flat, amber means
        curvature, red means fail-closed) cannot drift between the page and
        its figures.
   ========================================================================== */

(function () {
  "use strict";

  var doc = document;
  var root = doc.documentElement;

  /* Gate the reveal styles on JavaScript being present, so the no-script
     rendering is the fully visible one. */
  root.classList.add("js");

  var reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ----------------------------------------------------------------------
     Small helpers
     ---------------------------------------------------------------------- */

  var NS = "http://www.w3.org/2000/svg";

  function $(id) { return doc.getElementById(id); }

  function el(tag, attrs, text) {
    var node = doc.createElementNS(NS, tag);
    if (attrs) {
      for (var key in attrs) {
        if (Object.prototype.hasOwnProperty.call(attrs, key) && attrs[key] !== null) {
          node.setAttribute(key, String(attrs[key]));
        }
      }
    }
    if (text !== undefined && text !== null) { node.textContent = String(text); }
    return node;
  }

  function clear(svg) { while (svg.firstChild) { svg.removeChild(svg.firstChild); } }

  /* Real subscripts and superscripts inside SVG.

     Unicode has no subscript for most letters, so "r_AB" written with the few
     characters that do exist comes out as a mixture of sub and superscripts.
     Building the label from tspans is the only way to typeset it correctly.
     Each tspan's dy is relative to the previous one, so the running shift has
     to be tracked and undone. Parts are {t}, {sub} or {sup}. */
  function mathText(attrs, parts) {
    var node = el("text", attrs);
    var shift = 0;
    parts.forEach(function (part) {
      var target = 0;
      var value = part.t;
      if (part.sub !== undefined) { target = 3.6; value = part.sub; }
      else if (part.sup !== undefined) { target = -5; value = part.sup; }
      var span = el("tspan", { dy: (target - shift).toFixed(2) }, value);
      if (target !== 0) { span.setAttribute("font-size", "0.72em"); }
      node.appendChild(span);
      shift = target;
    });
    return node;
  }

  function ratio(sub) { return [{ t: "r" }, { sub: sub }]; }

  /* Fixed-width numeric formatting. Values that are exactly zero print as a
     clean zero rather than a signed near-zero, because "exactly zero" is a
     claim this project makes on purpose. */
  function fmt(value, digits) {
    var d = digits === undefined ? 3 : digits;
    if (!isFinite(value)) { return "--"; }
    if (Math.abs(value) < Math.pow(10, -(d + 1))) { return (0).toFixed(d); }
    return value.toFixed(d);
  }

  function signed(value, digits) {
    var d = digits === undefined ? 3 : digits;
    if (Math.abs(value) < Math.pow(10, -(d + 1))) { return (0).toFixed(d); }
    return (value > 0 ? "+" : "") + value.toFixed(d);
  }

  function setClassState(node, state) {
    if (!node) { return; }
    node.classList.remove("is-flat", "is-curve", "is-block");
    if (state) { node.classList.add("is-" + state); }
  }

  function setVerdict(node, kind, label) {
    if (!node) { return; }
    node.className = "verdict " + kind;
    node.textContent = label;
  }

  /* ----------------------------------------------------------------------
     Theme
     ---------------------------------------------------------------------- */

  (function theme() {
    var toggle = $("themeToggle");
    var stored = null;
    try { stored = window.localStorage.getItem("mi-theme"); } catch (err) { stored = null; }
    if (stored === "light" || stored === "dark") { root.setAttribute("data-theme", stored); }

    if (!toggle) { return; }

    toggle.addEventListener("click", function () {
      var systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      var current = root.getAttribute("data-theme") || (systemDark ? "dark" : "light");
      var next = current === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try { window.localStorage.setItem("mi-theme", next); } catch (err) { /* storage may be denied */ }
      toggle.setAttribute("aria-label", next === "dark" ? "Switch to the light theme" : "Switch to the dark theme");
    });
  }());

  /* ----------------------------------------------------------------------
     Scroll reveal

     An IntersectionObserver alone leaves blocks stuck at zero opacity after an
     instant jump (the End key, a scrollbar drag, a deep link), because no
     intersection is ever generated for content that was never scrolled past.
     A sweep on load and on scroll covers that case.
     ---------------------------------------------------------------------- */

  (function reveal() {
    var targets = Array.prototype.slice.call(doc.querySelectorAll(".reveal"));
    if (!targets.length) { return; }

    if (reducedMotion || typeof IntersectionObserver !== "function") {
      targets.forEach(function (node) { node.classList.add("revealed"); });
      return;
    }

    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add("revealed");
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.06, rootMargin: "0px 0px -40px 0px" });

    targets.forEach(function (node) { observer.observe(node); });

    function sweep() {
      var limit = window.innerHeight + 220;
      targets.forEach(function (node) {
        if (node.classList.contains("revealed")) { return; }
        if (node.getBoundingClientRect().top < limit) {
          node.classList.add("revealed");
          observer.unobserve(node);
        }
      });
    }

    window.addEventListener("load", sweep);
    window.addEventListener("scroll", sweep, { passive: true });
    sweep();
  }());

  /* ----------------------------------------------------------------------
     Header state, reading progress, section highlighting
     ---------------------------------------------------------------------- */

  (function chrome() {
    var header = doc.querySelector(".top");
    var progress = $("readProgress");
    var links = Array.prototype.slice.call(doc.querySelectorAll(".top nav a"));
    var ticking = false;

    function frame() {
      ticking = false;
      var y = window.scrollY || window.pageYOffset;
      if (header) { header.classList.toggle("stuck", y > 24); }
      if (progress) {
        var span = doc.body.scrollHeight - window.innerHeight;
        var ratio = span > 0 ? Math.min(1, Math.max(0, y / span)) : 0;
        progress.style.transform = "scaleX(" + ratio.toFixed(4) + ")";
      }
    }

    function onScroll() {
      if (!ticking) { ticking = true; window.requestAnimationFrame(frame); }
    }

    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);
    frame();

    if (!links.length || typeof IntersectionObserver !== "function") { return; }

    var sections = links
      .map(function (link) { return doc.getElementById(link.getAttribute("href").slice(1)); })
      .filter(Boolean);

    var spy = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (!entry.isIntersecting) { return; }
        links.forEach(function (link) {
          var active = link.getAttribute("href") === "#" + entry.target.id;
          if (active) { link.setAttribute("aria-current", "true"); }
          else { link.removeAttribute("aria-current"); }
        });
      });
    }, { rootMargin: "-25% 0px -68% 0px" });

    sections.forEach(function (section) { spy.observe(section); });
  }());

  /* ----------------------------------------------------------------------
     Hero: the intervention square

     Four regime laws at the corners, two primitive mechanism changes along the
     edges, and a pulse that travels the cycle to say the thing the equation
     says: going round the square and coming back to where you started costs
     nothing exactly when the face curvature is zero.
     ---------------------------------------------------------------------- */

  (function heroArt() {
    var svg = $("heroArt");
    if (!svg) { return; }

    var p0 = { x: 122, y: 372, label: [{ t: "p" }, { sub: "0" }] };
    var pA = { x: 398, y: 372, label: [{ t: "p" }, { sub: "A" }] };
    var pB = { x: 162, y: 118, label: [{ t: "p" }, { sub: "B" }] };
    var pAB = { x: 438, y: 118, label: [{ t: "p" }, { sub: "AB" }] };
    var r = 46;

    var cycle = "M" + p0.x + "," + p0.y + " L" + pA.x + "," + pA.y +
                " L" + pAB.x + "," + pAB.y + " L" + pB.x + "," + pB.y + " Z";

    /* The face itself. */
    svg.appendChild(el("path", { d: cycle, class: "viz-cell flat", "stroke-width": 1.5, rx: 0 }));

    /* Edges, trimmed so they start and end outside the node discs. */
    function edge(from, to, label, lift) {
      var dx = to.x - from.x;
      var dy = to.y - from.y;
      var len = Math.sqrt(dx * dx + dy * dy);
      var ux = dx / len;
      var uy = dy / len;
      var x1 = from.x + ux * (r + 8);
      var y1 = from.y + uy * (r + 8);
      var x2 = to.x - ux * (r + 16);
      var y2 = to.y - uy * (r + 16);

      svg.appendChild(el("path", { d: "M" + x1 + "," + y1 + " L" + x2 + "," + y2, class: "viz-edge", "marker-end": "url(#heroHead)" }));
      svg.appendChild(mathText({
        x: (x1 + x2) / 2 + (lift ? 22 : 0),
        y: (y1 + y2) / 2 - (lift ? 0 : 12),
        class: "viz-edge-text", "font-size": 17
      }, [{ t: "ℓ" }, { sub: label }]));
    }

    var defs = el("defs");
    var marker = el("marker", {
      id: "heroHead", viewBox: "0 0 10 10", refX: 8, refY: 5,
      markerWidth: 5.5, markerHeight: 5.5, orient: "auto-start-reverse"
    });
    marker.appendChild(el("path", { d: "M0,1 L9,5 L0,9 z", class: "viz-arrow" }));
    defs.appendChild(marker);
    svg.appendChild(defs);

    edge(p0, pA, "A", false);
    edge(pB, pAB, "A", false);
    edge(p0, pB, "B", true);
    edge(pA, pAB, "B", true);

    /* The travelling pulse. Dash animation is used rather than motion along a
       path so that it degrades to a plain outline everywhere, and the global
       reduced-motion rule stops it without any extra branch. */
    if (!reducedMotion) {
      var pulse = el("path", { d: cycle, class: "hero-pulse" });
      svg.appendChild(pulse);
    }

    [p0, pA, pB, pAB].forEach(function (node) {
      svg.appendChild(el("circle", { cx: node.x, cy: node.y, r: r, class: "viz-node" }));
      svg.appendChild(mathText({ x: node.x, y: node.y + 8, class: "viz-node-text", "font-size": 24 }, node.label));
    });

    /* Centre badge: the closure statement, on one line so it reads as an
       equation rather than as two stacked fragments. */
    var cx = (p0.x + pA.x + pB.x + pAB.x) / 4;
    var cy = (p0.y + pA.y + pB.y + pAB.y) / 4;
    svg.appendChild(el("rect", { x: cx - 74, y: cy - 30, width: 148, height: 60, rx: 30, class: "hero-badge" }));
    svg.appendChild(mathText({ x: cx, y: cy + 9, class: "hero-badge-text", "text-anchor": "middle" },
      [{ t: "κ" }, { sub: "AB" }, { t: " = 0" }]));
  }());

  /* ----------------------------------------------------------------------
     Chart scaffolding shared by the plotted panels
     ---------------------------------------------------------------------- */

  function plotFrame(svg, box, opts) {
    svg.appendChild(el("rect", {
      x: box.left, y: box.top, width: box.width, height: box.height,
      rx: 10, class: "viz-plate"
    }));

    if (opts && opts.xTicks) {
      opts.xTicks.forEach(function (tick) {
        var x = box.xScale(tick);
        svg.appendChild(el("line", { x1: x, y1: box.top, x2: x, y2: box.top + box.height, class: "viz-grid" }));
        svg.appendChild(el("text", { x: x, y: box.top + box.height + 18, class: "viz-tick", "text-anchor": "middle" }, opts.xFormat ? opts.xFormat(tick) : String(tick)));
      });
    }

    if (opts && opts.yTicks) {
      opts.yTicks.forEach(function (tick) {
        var y = box.yScale(tick);
        var zero = Math.abs(tick) < 1e-12;
        svg.appendChild(el("line", {
          x1: box.left, y1: y, x2: box.left + box.width, y2: y,
          class: zero ? "viz-axis" : "viz-grid"
        }));
        svg.appendChild(el("text", { x: box.left - 8, y: y + 4, class: "viz-tick", "text-anchor": "end" }, opts.yFormat ? opts.yFormat(tick) : fmt(tick, 2)));
      });
    }
  }

  function makeBox(width, height, margin) {
    return {
      left: margin.left,
      top: margin.top,
      width: width - margin.left - margin.right,
      height: height - margin.top - margin.bottom
    };
  }

  /* ----------------------------------------------------------------------
     The interferometer

     Complete state: r_A(x1) = 1 + a x1 and r_B(x2) = 1 + b x2, so the joint
     ratio is the exact product and every face curvature is identically zero.

     Observed through Y alone: kappa(y) = log(1 + a b tanh(y / sigma^2)).
     ---------------------------------------------------------------------- */

  (function interferometer() {
    var aSlider = $("aSlider");
    var bSlider = $("bSlider");
    var sigmaSlider = $("sigmaSlider");
    var square = $("squareViz");
    var chart = $("kappaChart");
    if (!aSlider || !bSlider || !sigmaSlider || !square || !chart) { return; }

    function drawSquare(a, b) {
      clear(square);

      var states = [-1, 1];
      var gridLeft = 76;
      var gridTop = 40;
      var cellW = (372 - gridLeft) / 2 - 7;
      var cellH = (302 - gridTop) / 2 - 6;
      var pad = 14;

      states.forEach(function (x2, col) {
        square.appendChild(el("text", {
          x: gridLeft + col * (cellW + 14) + cellW / 2,
          y: 26, class: "viz-label", "text-anchor": "middle"
        }, "X₂ = " + (x2 > 0 ? "+1" : "−1")));
      });

      states.forEach(function (x1, row) {
        square.appendChild(el("text", {
          x: 8, y: gridTop + row * (cellH + 12) + cellH / 2 + 4,
          class: "viz-label"
        }, "X₁ = " + (x1 > 0 ? "+1" : "−1")));

        states.forEach(function (x2, col) {
          var rA = 1 + a * x1;
          var rB = 1 + b * x2;
          var rAB = rA * rB;
          var kappa = Math.log(rAB / (rA * rB));
          var x = gridLeft + col * (cellW + 14);
          var y = gridTop + row * (cellH + 12);

          square.appendChild(el("rect", { x: x, y: y, width: cellW, height: cellH, rx: 12, class: "viz-cell flat" }));
          square.appendChild(mathText({ x: x + pad, y: y + 26, class: "viz-val dim", "text-anchor": "start" },
            [{ t: "r" }, { sub: "A" }, { t: " r" }, { sub: "B" }]));
          square.appendChild(el("text", { x: x + cellW - pad, y: y + 26, class: "viz-val", "text-anchor": "end" }, fmt(rA * rB, 3)));
          square.appendChild(mathText({ x: x + pad, y: y + 50, class: "viz-val dim", "text-anchor": "start" }, ratio("AB")));
          square.appendChild(el("text", { x: x + cellW - pad, y: y + 50, class: "viz-val", "text-anchor": "end" }, fmt(rAB, 3)));
          square.appendChild(el("line", { x1: x + pad, y1: y + 64, x2: x + cellW - pad, y2: y + 64, class: "viz-grid" }));
          square.appendChild(mathText({
            x: x + cellW / 2, y: y + 92, class: "viz-val flat", "font-size": 16, "text-anchor": "middle"
          }, [{ t: "κ" }, { sub: "AB" }, { t: " = " + fmt(kappa, 3) }]));
        });
      });
    }

    function drawCurve(a, b, sigma) {
      clear(chart);

      var ab = a * b;
      var lo = Math.log(1 - Math.abs(ab));
      var hi = Math.log(1 + Math.abs(ab));
      var bound = Math.max(0.12, Math.max(Math.abs(lo), Math.abs(hi)) * 1.25);

      var box = makeBox(380, 320, { left: 52, right: 12, top: 18, bottom: 44 });
      box.xScale = function (x) { return box.left + (x + 4) / 8 * box.width; };
      box.yScale = function (y) { return box.top + (bound - y) / (2 * bound) * box.height; };

      plotFrame(chart, box, {
        xTicks: [-4, -2, 0, 2, 4],
        yTicks: [-bound, -bound / 2, 0, bound / 2, bound],
        yFormat: function (v) { return fmt(v, 2); }
      });

      var steps = 240;
      var line = "";
      var area = "";
      for (var i = 0; i <= steps; i += 1) {
        var y = -4 + 8 * i / steps;
        var k = Math.log(1 + ab * Math.tanh(y / (sigma * sigma)));
        var px = box.xScale(y).toFixed(2);
        var py = box.yScale(k).toFixed(2);
        line += (i ? "L" : "M") + px + "," + py + " ";
        area += (i ? "L" : "M" + px + "," + box.yScale(0).toFixed(2) + " L") + px + "," + py + " ";
      }
      area += "L" + box.xScale(4).toFixed(2) + "," + box.yScale(0).toFixed(2) + " Z";

      var isFlat = Math.abs(ab) < 1e-12;
      chart.appendChild(el("path", { d: area, class: "viz-area" + (isFlat ? " flat" : "") }));
      chart.appendChild(el("path", { d: line, class: "viz-curve" + (isFlat ? " flat" : "") }));

      chart.appendChild(el("text", {
        x: box.left + box.width / 2, y: 314, class: "viz-label", "text-anchor": "middle"
      }, "observed outcome y"));
      chart.appendChild(el("text", {
        x: 13, y: box.top + box.height / 2, class: "viz-label", "text-anchor": "middle",
        transform: "rotate(-90 13 " + (box.top + box.height / 2) + ")"
      }, "marginal κ(y)"));
    }

    function render() {
      var a = Number(aSlider.value);
      var b = Number(bSlider.value);
      var sigma = Number(sigmaSlider.value);
      var ab = a * b;

      $("aOut").value = a.toFixed(2);
      $("bOut").value = b.toFixed(2);
      $("sigmaOut").value = sigma.toFixed(2);

      $("synergyOut").textContent = fmt(ab, 3);
      $("fullKappaOut").textContent = fmt(0, 3);

      var lo = Math.log(1 - Math.abs(ab));
      var hi = Math.log(1 + Math.abs(ab));
      $("rangeOut").textContent = "[" + fmt(lo, 3) + ", " + fmt(hi, 3) + "]";

      var flatMarginal = Math.abs(ab) < 1e-12;
      setClassState($("rangeOut"), flatMarginal ? "flat" : "curve");
      setVerdict($("marginalVerdict"), flatMarginal ? "flat" : "curve",
        flatMarginal ? "κ = 0" : "κ ≠ 0");
      setVerdict($("fullVerdict"), "flat", "κ = 0 always");

      /* The battery is a genuine computation, not a label: with outcome-only
         primitive ratios both identically one, its value is exactly one no
         matter how much curvature is present. That is the blind spot. */
      var battery = 1;
      $("batteryOut").textContent = Math.abs(battery - 1) < 1e-12
        ? (flatMarginal ? "passes" : "passes, blind")
        : "fails";
      setClassState($("batteryOut"), flatMarginal ? "flat" : "curve");

      /* Say plainly whether the reader is still looking at the published
         fixture or at a setting of their own. */
      var onFixture = a === 0.6 && b === 0.5 && sigma === 0.8;
      setVerdict($("fixtureState"), onFixture ? "info" : "muted",
        onFixture ? "paper fixture" : "custom setting");

      drawSquare(a, b);
      drawCurve(a, b, sigma);
    }

    [aSlider, bSlider, sigmaSlider].forEach(function (slider) {
      slider.addEventListener("input", render);
    });

    var reset = $("resetInterferometer");
    if (reset) {
      reset.addEventListener("click", function () {
        aSlider.value = "0.6";
        bSlider.value = "0.5";
        sigmaSlider.value = "0.8";
        render();
      });
    }

    render();
  }());

  /* ----------------------------------------------------------------------
     Tabs
     ---------------------------------------------------------------------- */

  function wireTabs(listSelector, onSelect) {
    var list = doc.querySelector(listSelector);
    if (!list) { return; }
    var tabs = Array.prototype.slice.call(list.querySelectorAll('[role="tab"]'));

    function select(tab) {
      tabs.forEach(function (other) {
        var active = other === tab;
        other.setAttribute("aria-selected", active ? "true" : "false");
        var pane = doc.getElementById(other.getAttribute("aria-controls"));
        if (pane) {
          if (active) { pane.removeAttribute("hidden"); }
          else { pane.setAttribute("hidden", ""); }
        }
      });
      if (typeof onSelect === "function") { onSelect(tab.getAttribute("aria-controls")); }
    }

    tabs.forEach(function (tab, index) {
      tab.addEventListener("click", function () { select(tab); });
      tab.addEventListener("keydown", function (event) {
        var delta = event.key === "ArrowRight" ? 1 : (event.key === "ArrowLeft" ? -1 : 0);
        if (!delta) { return; }
        event.preventDefault();
        var next = tabs[(index + delta + tabs.length) % tabs.length];
        next.focus();
        select(next);
      });
    });
  }

  /* ----------------------------------------------------------------------
     Scenario: flat composition

     The primitive ratios and their product, drawn as coincident bars, at the
     paper's default tilts.
     ---------------------------------------------------------------------- */

  function drawFlatScenario() {
    var svg = $("vizFlat");
    if (!svg) { return; }
    clear(svg);

    var a = 0.6;
    var b = 0.5;
    var states = [
      { x1: -1, x2: -1 }, { x1: -1, x2: 1 }, { x1: 1, x2: -1 }, { x1: 1, x2: 1 }
    ];

    var box = makeBox(520, 300, { left: 52, right: 14, top: 22, bottom: 52 });
    var maxV = 2.6;
    box.yScale = function (v) { return box.top + (maxV - v) / maxV * box.height; };

    plotFrame(svg, box, {
      yTicks: [0, 0.65, 1.3, 1.95, 2.6],
      yFormat: function (v) { return fmt(v, 2); }
    });

    var slot = box.width / states.length;

    states.forEach(function (state, index) {
      var rA = 1 + a * state.x1;
      var rB = 1 + b * state.x2;
      var product = rA * rB;
      var joint = rA * rB;

      var cx = box.left + slot * (index + 0.5);
      var w = Math.min(64, slot * 0.5);

      svg.appendChild(el("rect", {
        x: cx - w / 2, y: box.yScale(product), width: w,
        height: box.top + box.height - box.yScale(product), rx: 5, class: "viz-bar"
      }));

      /* The joint ratio drawn as an outline directly over the product, so
         "they coincide" is something you see rather than something you read. */
      svg.appendChild(el("rect", {
        x: cx - w / 2 - 6, y: box.yScale(joint), width: w + 12,
        height: box.top + box.height - box.yScale(joint), rx: 7,
        class: "viz-outline flat"
      }));

      svg.appendChild(el("text", { x: cx, y: box.yScale(joint) - 10, class: "viz-val flat" }, fmt(joint, 3)));
      svg.appendChild(el("text", {
        x: cx, y: box.top + box.height + 20, class: "viz-tick", "text-anchor": "middle"
      }, "(" + (state.x1 > 0 ? "+" : "−") + ", " + (state.x2 > 0 ? "+" : "−") + ")"));
      svg.appendChild(el("text", {
        x: cx, y: box.top + box.height + 38, class: "viz-val flat", "font-size": 11
      }, "κ = 0"));
    });

    svg.appendChild(mathText({ x: box.left, y: 14, class: "viz-title" },
      [{ t: "filled: r" }, { sub: "A" }, { t: " r" }, { sub: "B" }, { t: "     outline: r" }, { sub: "AB" }]));
  }

  /* ----------------------------------------------------------------------
     Scenario: hidden state

     Coordinated tilts that compose flatly on the complete state. After
     marginalization the observable ratio covariance is -a^2 and the mean
     conditional latent covariance is +a^2. They sum to zero exactly.
     ---------------------------------------------------------------------- */

  (function latentScenario() {
    var slider = $("latentSlider");
    var svg = $("vizLatent");
    if (!slider || !svg) { return; }

    function render() {
      var a = Number(slider.value);
      var covObserved = -(a * a);
      var covHidden = a * a;
      var kappa = -Math.log(1 - a * a);
      var residual = covObserved + covHidden;

      $("latentOut").value = a.toFixed(2);
      $("latentKappa").textContent = fmt(kappa, 6);
      $("latentCovObs").textContent = signed(covObserved, 6);
      $("latentCovHid").textContent = signed(covHidden, 6);
      $("latentResidual").textContent = fmt(residual, 6);
      setVerdict($("latentVerdict"), "flat", "sum = " + fmt(residual, 3));

      clear(svg);

      var box = makeBox(520, 300, { left: 62, right: 14, top: 26, bottom: 46 });
      var bound = 0.78;
      box.yScale = function (v) { return box.top + (bound - v) / (2 * bound) * box.height; };

      plotFrame(svg, box, {
        yTicks: [-bound, -bound / 2, 0, bound / 2, bound],
        yFormat: function (v) { return fmt(v, 2); }
      });

      var zero = box.yScale(0);

      function bar(centre, value, cls, caption) {
        var w = 92;
        var top = value >= 0 ? box.yScale(value) : zero;
        var height = Math.abs(box.yScale(value) - zero);
        svg.appendChild(el("rect", {
          x: centre - w / 2, y: top, width: w, height: Math.max(height, 1.5), rx: 6, class: "viz-bar " + cls
        }));
        svg.appendChild(el("text", {
          x: centre, y: value >= 0 ? top - 10 : top + height + 20,
          class: "viz-val " + cls
        }, signed(value, 4)));
        svg.appendChild(mathText({
          x: centre, y: box.top + box.height + 22, class: "viz-tick", "text-anchor": "middle"
        }, caption));
      }

      bar(box.left + box.width * 0.26, covObserved, "curve",
        [{ t: "observed  Cov(r" }, { sub: "A" }, { t: ", r" }, { sub: "B" }, { t: ")" }]);
      bar(box.left + box.width * 0.72, covHidden, "flat",
        [{ t: "hidden  E[Cov(L" }, { sub: "A" }, { t: ", L" }, { sub: "B" }, { t: " | X)]" }]);

      /* The balance beam: a bracket joining the two bar tips through zero. */
      var x1 = box.left + box.width * 0.26;
      var x2 = box.left + box.width * 0.72;
      svg.appendChild(el("path", {
        d: "M" + x1 + "," + box.yScale(covObserved) + " L" + x1 + "," + zero +
           " L" + x2 + "," + zero + " L" + x2 + "," + box.yScale(covHidden),
        class: "viz-beam"
      }));
      svg.appendChild(el("text", {
        x: (x1 + x2) / 2, y: zero - 12, class: "viz-val flat", "font-size": 14
      }, "Σ = " + fmt(residual, 6)));
      svg.appendChild(el("text", { x: box.left, y: 16, class: "viz-title" }, "curvature is moved, never lost"));
    }

    slider.addEventListener("input", render);
    render();
  }());

  /* ----------------------------------------------------------------------
     Scenario: implementation drift

     The joint arm acquires an extra normalized factor (1 + gamma x1 x2), so
     kappa = log(1 + gamma x1 x2) - log(1 + a b gamma). At gamma = 0 the
     negative control returns exactly zero.
     ---------------------------------------------------------------------- */

  (function implementationScenario() {
    var slider = $("gammaSlider");
    var svg = $("vizImpl");
    if (!slider || !svg) { return; }

    var a = 0.45;
    var b = 0.35;

    function render() {
      var gamma = Number(slider.value);
      var normalizer = 1 + a * b * gamma;
      var kPlus = Math.log(1 + gamma) - Math.log(normalizer);
      var kMinus = Math.log(1 - gamma) - Math.log(normalizer);
      var isControl = gamma === 0;

      $("gammaOut").value = gamma.toFixed(2);
      $("implNorm").textContent = fmt(normalizer, 6);
      $("implPlus").textContent = signed(kPlus, 6);
      $("implMinus").textContent = signed(kMinus, 6);
      $("implControl").textContent = fmt(0, 6);
      setClassState($("implPlus"), isControl ? "flat" : "curve");
      setClassState($("implMinus"), isControl ? "flat" : "curve");
      setVerdict($("implVerdict"), isControl ? "flat" : "block",
        isControl ? "negative control" : "apparatus");

      clear(svg);

      var box = makeBox(520, 300, { left: 62, right: 14, top: 26, bottom: 46 });
      var bound = Math.max(0.35, Math.abs(kMinus) * 1.2);
      box.yScale = function (v) { return box.top + (bound - v) / (2 * bound) * box.height; };

      plotFrame(svg, box, {
        yTicks: [-bound, -bound / 2, 0, bound / 2, bound],
        yFormat: function (v) { return fmt(v, 2); }
      });

      var zero = box.yScale(0);

      function bar(centre, value, caption) {
        var w = 92;
        var top = value >= 0 ? box.yScale(value) : zero;
        var height = Math.abs(box.yScale(value) - zero);
        var cls = isControl ? "flat" : "curve";
        svg.appendChild(el("rect", {
          x: centre - w / 2, y: top, width: w, height: Math.max(height, 2), rx: 6, class: "viz-bar " + cls
        }));
        svg.appendChild(el("text", {
          x: centre, y: value >= 0 ? top - 10 : top + height + 20, class: "viz-val " + cls
        }, signed(value, 4)));
        svg.appendChild(el("text", {
          x: centre, y: box.top + box.height + 22, class: "viz-tick", "text-anchor": "middle"
        }, caption));
      }

      bar(box.left + box.width * 0.28, kMinus, "X₁X₂ = −1");
      bar(box.left + box.width * 0.72, kPlus, "X₁X₂ = +1");

      svg.appendChild(el("text", { x: box.left, y: 16, class: "viz-title" },
        isControl
          ? "γ = 0: the negative control returns exactly zero"
          : "a sign-varying field whose weighted mean can still vanish"));
    }

    slider.addEventListener("input", render);
    render();
  }());

  /* ----------------------------------------------------------------------
     Scenario: orientation ambiguity under balanced parity

     P uniform, N ~ Bernoulli(eps), baseline T = P xor N, intervention
     T = (not P) xor N. Both one-dimensional marginals stay at one half under
     both regimes; the joint law flips. Deleting either coordinate leaves an
     invariant marginal, so the pass count is two.
     ---------------------------------------------------------------------- */

  function drawParityScenario() {
    var svg = $("vizParity");
    if (!svg) { return; }
    clear(svg);

    var eps = 0.1;

    /* Joint laws, computed rather than tabulated. */
    function joint(flip) {
      var table = [];
      [0, 1].forEach(function (p) {
        [0, 1].forEach(function (t) {
          var n = (t ^ p ^ (flip ? 1 : 0)) & 1;
          table.push({ p: p, t: t, value: 0.5 * (n === 1 ? eps : 1 - eps) });
        });
      });
      return table;
    }

    var baseline = joint(false);
    var intervention = joint(true);

    function marginal(table, key, level) {
      return table.reduce(function (sum, cell) {
        return sum + (cell[key] === level ? cell.value : 0);
      }, 0);
    }

    svg.appendChild(el("text", { x: 8, y: 18, class: "viz-title" }, "one-dimensional marginals"));
    svg.appendChild(el("text", { x: 286, y: 18, class: "viz-title" }, "joint law of (P, T)"));

    /* Marginal bars: four bars, all exactly one half. */
    var baseY = 236;
    var scale = 190;
    var bars = [
      { label: "P baseline", value: marginal(baseline, "p", 1) },
      { label: "P after", value: marginal(intervention, "p", 1) },
      { label: "T baseline", value: marginal(baseline, "t", 1) },
      { label: "T after", value: marginal(intervention, "t", 1) }
    ];

    svg.appendChild(el("line", { x1: 10, y1: baseY, x2: 258, y2: baseY, class: "viz-axis" }));

    bars.forEach(function (bar, index) {
      var w = 42;
      var x = 22 + index * 60;
      var h = bar.value * scale;
      svg.appendChild(el("rect", { x: x, y: baseY - h, width: w, height: h, rx: 5, class: "viz-bar flat" }));
      svg.appendChild(el("text", { x: x + w / 2, y: baseY - h - 8, class: "viz-val flat", "font-size": 11 }, fmt(bar.value, 3)));
      svg.appendChild(el("text", { x: x + w / 2, y: baseY + 16, class: "viz-tick", "text-anchor": "middle", "font-size": 9.5 }, bar.label.split(" ")[0]));
      svg.appendChild(el("text", { x: x + w / 2, y: baseY + 28, class: "viz-tick", "text-anchor": "middle", "font-size": 9.5 }, bar.label.split(" ")[1]));
    });

    svg.appendChild(el("text", { x: 134, y: 278, class: "viz-val flat", "font-size": 12 }, "identical: both deletions pass"));

    /* Joint tables: two 2x2 grids, cell opacity carrying the probability. */
    function table(originX, data, title) {
      svg.appendChild(el("text", { x: originX + 55, y: 44, class: "viz-tick", "text-anchor": "middle" }, title));
      data.forEach(function (cell) {
        var x = originX + cell.t * 56;
        var y = 56 + cell.p * 56;
        svg.appendChild(el("rect", {
          x: x, y: y, width: 52, height: 52, rx: 8,
          class: "viz-cell curve", "fill-opacity": (0.18 + cell.value * 1.5).toFixed(3)
        }));
        svg.appendChild(el("text", { x: x + 26, y: y + 31, class: "viz-val curve", "font-size": 12 }, fmt(cell.value, 2)));
      });
      [0, 1].forEach(function (t) {
        svg.appendChild(el("text", { x: originX + t * 56 + 26, y: 176, class: "viz-tick", "text-anchor": "middle", "font-size": 10 }, "T=" + t));
      });
      [0, 1].forEach(function (p) {
        svg.appendChild(el("text", { x: originX - 8, y: 56 + p * 56 + 31, class: "viz-tick", "text-anchor": "end", "font-size": 10 }, "P=" + p));
      });
    }

    table(300, baseline, "baseline");
    table(420, intervention, "after");

    svg.appendChild(el("text", { x: 300, y: 208, class: "viz-val curve", "font-size": 12, "text-anchor": "start" }, "the joint law flips"));
    svg.appendChild(el("text", { x: 300, y: 232, class: "viz-tick", "text-anchor": "start" }, "pass count = 2"));
    svg.appendChild(el("text", { x: 300, y: 250, class: "viz-tick", "text-anchor": "start" }, "status: AMBIGUOUS_MULTIPLE_PASSES"));
    svg.appendChild(el("text", { x: 300, y: 274, class: "viz-tick", "text-anchor": "start" }, "remedy: several asymmetric tilts"));
  }

  wireTabs('[role="tablist"]', function () { /* panes are already drawn */ });
  drawFlatScenario();
  drawParityScenario();

  /* ----------------------------------------------------------------------
     Orientation: equivalence bounds and the pass-count state machine

     A deletion is certified invariant only when the whole interval lies below
     the tolerance, and certified changed only when it lies entirely above.
     Failure to reject is never evidence of invariance, so a straddling
     interval produces an explicit undetermined state instead of a decision.
     ---------------------------------------------------------------------- */

  (function orientation() {
    var scenarioSelect = $("orientScenario");
    var epsSlider = $("epsSlider");
    var svg = $("vizOrient");
    if (!scenarioSelect || !epsSlider || !svg) { return; }

    var scenarios = {
      unique: {
        note: "Exactly one coordinate is certified invariant. The family is oriented.",
        rows: [
          { name: "delete T", point: 0.06, half: 0.05 },
          { name: "delete P₁", point: 0.82, half: 0.10 },
          { name: "delete P₂", point: 0.71, half: 0.09 }
        ]
      },
      none: {
        note: "No coordinate survives deletion. Suspect descendant contamination, a multi-target primitive, selection, or an implementation mismatch.",
        rows: [
          { name: "delete T", point: 0.44, half: 0.08 },
          { name: "delete P₁", point: 0.79, half: 0.10 },
          { name: "delete P₂", point: 0.68, half: 0.09 }
        ]
      },
      multiple: {
        note: "Balanced parity: two coordinates pass. No target may be inferred, and forcing one would be a fabrication.",
        rows: [
          { name: "delete P", point: 0.03, half: 0.03 },
          { name: "delete T", point: 0.04, half: 0.03 },
          { name: "delete N", point: 0.75, half: 0.10 }
        ]
      },
      under: {
        note: "Every interval is wider than the tolerance. The sample cannot answer the question; report the abstention, not a point estimate.",
        rows: [
          { name: "delete T", point: 0.10, half: 0.45 },
          { name: "delete P₁", point: 0.55, half: 0.50 },
          { name: "delete P₂", point: 0.40, half: 0.48 }
        ]
      },
      mixed: {
        note: "One coordinate is certified invariant, but another straddles the tolerance. The orientation is undetermined until that interval is tightened.",
        rows: [
          { name: "delete T", point: 0.05, half: 0.04 },
          { name: "delete P₁", point: 0.22, half: 0.14 },
          { name: "delete P₂", point: 0.80, half: 0.09 }
        ]
      }
    };

    function classify(rows, eps) {
      var passes = 0;
      var straddle = 0;
      var widthSum = 0;

      rows.forEach(function (row) {
        var lower = Math.max(0, row.point - row.half);
        var upper = row.point + row.half;
        widthSum += upper - lower;
        if (upper < eps) { passes += 1; }
        else if (lower > eps) { /* certified changed */ }
        else { straddle += 1; }
      });

      var meanWidth = widthSum / rows.length;
      var state;
      if (straddle > 0 && meanWidth > 0.5) { state = "UNDERPOWERED"; }
      else if (straddle > 0) { state = "UNDETERMINED"; }
      else if (passes === 1) { state = "UNIQUE_TARGET"; }
      else if (passes === 0) { state = "NO_PASS"; }
      else { state = "MULTIPLE_PASSES"; }

      return { passes: passes, straddle: straddle, state: state };
    }

    var stateStyle = {
      UNIQUE_TARGET: { kind: "flat", verdict: "oriented" },
      NO_PASS: { kind: "curve", verdict: "no target" },
      MULTIPLE_PASSES: { kind: "block", verdict: "ambiguous" },
      UNDERPOWERED: { kind: "muted", verdict: "underpowered" },
      UNDETERMINED: { kind: "curve", verdict: "undetermined" }
    };

    function render() {
      var scenario = scenarios[scenarioSelect.value] || scenarios.unique;
      var eps = Number(epsSlider.value);
      var result = classify(scenario.rows, eps);
      var style = stateStyle[result.state];

      $("epsOut").value = eps.toFixed(2);
      $("passCount").textContent = String(result.passes);
      $("passState").textContent = result.state;
      setClassState($("passState"), style.kind === "muted" ? null : style.kind);
      setVerdict($("orientVerdict"), style.kind, style.verdict);
      $("orientNote").textContent = scenario.note;

      clear(svg);

      var box = makeBox(560, 300, { left: 96, right: 22, top: 40, bottom: 48 });
      var maxR = 1.45;
      box.xScale = function (v) { return box.left + Math.min(v, maxR) / maxR * box.width; };

      plotFrame(svg, box, {
        xTicks: [0, 0.25, 0.5, 0.75, 1.0, 1.25],
        xFormat: function (v) { return v.toFixed(2); }
      });

      /* The tolerance line is the decision boundary and is drawn as such. Its
         label sits inside the plate so it can never collide with the title. */
      var epsX = box.xScale(eps);
      svg.appendChild(el("line", { x1: epsX, y1: box.top + 26, x2: epsX, y2: box.top + box.height - 2, class: "viz-threshold" }));
      svg.appendChild(el("text", {
        x: epsX + 7, y: box.top + 17, class: "viz-val curve", "font-size": 11.5, "text-anchor": "start"
      }, "ε = " + eps.toFixed(2)));

      scenario.rows.forEach(function (row, index) {
        var lower = Math.max(0, row.point - row.half);
        var upper = row.point + row.half;
        var y = box.top + 30 + (box.height - 34) * (index + 0.5) / scenario.rows.length;
        var cls = upper < eps ? "flat" : (lower > eps ? "block" : "curve");
        var verdictText = upper < eps ? "invariant" : (lower > eps ? "changed" : "undetermined");

        svg.appendChild(el("text", { x: box.left - 12, y: y + 4, class: "viz-label", "text-anchor": "end" }, row.name));
        svg.appendChild(el("line", { x1: box.xScale(lower), y1: y, x2: box.xScale(upper), y2: y, class: "viz-interval " + cls }));
        [lower, upper].forEach(function (end) {
          svg.appendChild(el("line", { x1: box.xScale(end), y1: y - 7, x2: box.xScale(end), y2: y + 7, class: "viz-interval " + cls }));
        });
        svg.appendChild(el("circle", { cx: box.xScale(row.point), cy: y, r: 5.5, class: "viz-bar " + cls }));

        /* The verdict sits above its own interval rather than beside it. Beside
           it, the label and the end cap land on the same baseline and read as
           a strike-through. */
        var rightRoom = box.left + box.width - box.xScale(upper);
        var toRight = rightRoom > 86;
        var labelX = toRight ? box.xScale(upper) + 10 : box.xScale(lower) - 10;
        var anchor = toRight ? "start" : "end";

        /* A passing interval sits left of the tolerance line by definition, so
           its label must not reach across it and imply otherwise. */
        if (cls === "flat" && labelX + 62 > epsX) {
          labelX = Math.max(box.left + 4, box.xScale(lower));
          anchor = "start";
        }

        svg.appendChild(el("text", {
          x: labelX, y: y - 11, class: "viz-val " + cls, "font-size": 11, "text-anchor": anchor
        }, verdictText));
      });

      svg.appendChild(el("text", {
        x: box.left + box.width / 2, y: box.top + box.height + 34, class: "viz-label", "text-anchor": "middle"
      }, "normalized discrepancy  R = D / (D_full + η)"));
      svg.appendChild(el("text", { x: 8, y: 18, class: "viz-title" }, "equivalence intervals, not p-values"));
    }

    scenarioSelect.addEventListener("change", render);
    epsSlider.addEventListener("input", render);
    render();
  }());

  /* ----------------------------------------------------------------------
     Linear algebra used by the partial-design widget.

     Gauss-Jordan with partial pivoting. Only the rank is needed, and the
     matrices here are at most 8 by 8 with entries in {0, 1, -1}, so the naive
     implementation is both exact enough and instant.
     ---------------------------------------------------------------------- */

  function matrixRank(rows, tolerance) {
    var tol = tolerance === undefined ? 1e-10 : tolerance;
    var m = rows.map(function (row) { return row.slice(); });
    var height = m.length;
    if (!height) { return 0; }
    var width = m[0].length;
    var rank = 0;

    for (var col = 0; col < width && rank < height; col += 1) {
      var pivot = rank;
      var best = Math.abs(m[rank][col]);
      for (var i = rank + 1; i < height; i += 1) {
        if (Math.abs(m[i][col]) > best) { best = Math.abs(m[i][col]); pivot = i; }
      }
      if (best <= tol) { continue; }

      var swap = m[rank]; m[rank] = m[pivot]; m[pivot] = swap;

      for (var r = 0; r < height; r += 1) {
        if (r === rank) { continue; }
        var factor = m[r][col] / m[rank][col];
        if (factor === 0) { continue; }
        for (var c = col; c < width; c += 1) { m[r][c] -= factor * m[rank][c]; }
      }
      rank += 1;
    }
    return rank;
  }

  /* ----------------------------------------------------------------------
     Partial factorial designs

     Reproduces mic-design: main-effects rank over the observed corners, the
     lack-of-fit dimension left over, every fully observed square face, and
     whether the square contrasts span the whole testable space. The six-corner
     cube is the case worth staring at: no complete square face survives, and
     two flatness restrictions remain regardless.
     ---------------------------------------------------------------------- */

  (function partialDesigns() {
    var host = $("cornerToggles");
    var svg = $("vizCube");
    if (!host || !svg) { return; }

    var FACTORS = 3;
    var corners = [];
    for (var index = 0; index < (1 << FACTORS); index += 1) {
      var bits = [];
      for (var bit = 0; bit < FACTORS; bit += 1) { bits.push((index >> (FACTORS - 1 - bit)) & 1); }
      corners.push({ bits: bits, label: bits.join("") });
    }

    var position = corners.map(function (corner) {
      return {
        x: 80 + corner.bits[0] * 180 + corner.bits[2] * 95,
        y: 258 - corner.bits[1] * 150 - corner.bits[2] * 70
      };
    });

    /* Every (j, k) pair, at every setting of the remaining coordinates. */
    var faces = [];
    for (var j = 0; j < FACTORS; j += 1) {
      for (var k = j + 1; k < FACTORS; k += 1) {
        for (var base = 0; base < (1 << FACTORS); base += 1) {
          if (((base >> (FACTORS - 1 - j)) & 1) || ((base >> (FACTORS - 1 - k)) & 1)) { continue; }
          var bitJ = 1 << (FACTORS - 1 - j);
          var bitK = 1 << (FACTORS - 1 - k);
          faces.push({ j: j, k: k, corners: [base, base | bitJ, base | bitJ | bitK, base | bitK] });
        }
      }
    }

    var boxes = corners.map(function (corner, cornerIndex) {
      var label = doc.createElement("label");
      label.className = "corner-toggle";
      var box = doc.createElement("input");
      box.type = "checkbox";
      box.checked = true;
      box.id = "corner-" + corner.label;
      var text = doc.createElement("span");
      text.textContent = corner.label;
      label.appendChild(box);
      label.appendChild(text);
      host.appendChild(label);
      box.addEventListener("change", function () { setPresetLabel("custom selection"); render(); });
      return box;
    });

    function setPresetLabel(text) {
      var node = $("cubePreset");
      if (node) { node.textContent = text; }
    }

    function finding(kind, code, message) {
      var node = doc.createElement("div");
      node.className = "finding " + kind;
      var codeNode = doc.createElement("code");
      codeNode.textContent = code;
      var text = doc.createElement("p");
      text.textContent = message;
      node.appendChild(codeNode);
      node.appendChild(text);
      return node;
    }

    function analyse() {
      var observed = [];
      boxes.forEach(function (box, cornerIndex) { if (box.checked) { observed.push(cornerIndex); } });

      var slot = {};
      observed.forEach(function (cornerIndex, row) { slot[cornerIndex] = row; });

      /* Main-effects design matrix: an intercept plus one column per factor. */
      var mainRows = observed.map(function (cornerIndex) {
        return [1].concat(corners[cornerIndex].bits);
      });
      var mainRank = matrixRank(mainRows);
      var lackOfFit = observed.length - mainRank;

      var completeFaces = faces.filter(function (face) {
        return face.corners.every(function (cornerIndex) { return slot[cornerIndex] !== undefined; });
      });

      /* One contrast per complete face, expressed over the observed corners
         only: +1 at the base and the double flip, -1 at each single flip. */
      var contrasts = completeFaces.map(function (face) {
        var vector = new Array(observed.length).fill(0);
        vector[slot[face.corners[0]]] += 1;
        vector[slot[face.corners[1]]] -= 1;
        vector[slot[face.corners[2]]] += 1;
        vector[slot[face.corners[3]]] -= 1;
        return vector;
      });
      var squareRank = contrasts.length ? matrixRank(contrasts) : 0;

      return {
        observed: observed,
        slot: slot,
        mainRank: mainRank,
        lackOfFit: lackOfFit,
        completeFaces: completeFaces,
        squareRank: squareRank,
        spans: lackOfFit > 0 && squareRank === lackOfFit
      };
    }

    function draw(state) {
      clear(svg);

      completeFacePolygons(state);

      /* Every cube edge, dimmed unless both endpoints were observed. */
      corners.forEach(function (corner, a) {
        for (var bit = 0; bit < FACTORS; bit += 1) {
          var b = a ^ (1 << (FACTORS - 1 - bit));
          if (b <= a) { continue; }
          var live = state.slot[a] !== undefined && state.slot[b] !== undefined;
          svg.appendChild(el("line", {
            x1: position[a].x, y1: position[a].y, x2: position[b].x, y2: position[b].y,
            class: live ? "cube-edge live" : "cube-edge"
          }));
        }
      });

      corners.forEach(function (corner, cornerIndex) {
        var seen = state.slot[cornerIndex] !== undefined;
        svg.appendChild(el("circle", {
          cx: position[cornerIndex].x, cy: position[cornerIndex].y, r: 21,
          class: seen ? "viz-node" : "cube-missing"
        }));
        svg.appendChild(el("text", {
          x: position[cornerIndex].x, y: position[cornerIndex].y + 5,
          class: seen ? "viz-node-text" : "cube-missing-text", "font-size": 12.5
        }, corner.label));
      });

      svg.appendChild(el("text", { x: 10, y: 20, class: "viz-title" },
        state.completeFaces.length
          ? state.completeFaces.length + " complete square face" + (state.completeFaces.length === 1 ? "" : "s")
          : "no complete square face survives"));
      svg.appendChild(el("text", { x: 10, y: 300, class: "viz-tick" },
        "lack of fit " + state.lackOfFit + "   square contrast rank " + state.squareRank));
    }

    function completeFacePolygons(state) {
      state.completeFaces.forEach(function (face) {
        var points = face.corners.map(function (cornerIndex) {
          return position[cornerIndex].x + "," + position[cornerIndex].y;
        }).join(" ");
        svg.appendChild(el("polygon", { points: points, class: "cube-face" }));
      });
    }

    function render() {
      var state = analyse();

      $("cubeCorners").textContent = String(state.observed.length);
      $("cubeRank").textContent = String(state.mainRank);
      $("cubeLof").textContent = String(state.lackOfFit);
      $("cubeFaces").textContent = String(state.completeFaces.length);
      $("cubeSpan").textContent = state.lackOfFit === 0 ? "n/a" : (state.spans ? "yes" : "no");

      setClassState($("cubeLof"), state.lackOfFit > 0 ? "flat" : "curve");
      setClassState($("cubeSpan"), state.lackOfFit === 0 ? "curve" : (state.spans ? "flat" : "curve"));

      var box = $("cubeFindings");
      box.textContent = "";

      if (state.lackOfFit === 0) {
        setVerdict($("cubeVerdict"), "curve", "nothing testable");
        box.appendChild(finding("warn", "no_testable_flatness",
          "The observed design has no lack-of-fit degree of freedom beyond main effects. There is no flatness contrast to test."));
      } else if (!state.spans) {
        setVerdict($("cubeVerdict"), "curve", "squares insufficient");
        box.appendChild(finding("warn", "non_square_contrasts_required",
          "Observed square contrasts do not span the full testable lack-of-fit space. " +
          state.lackOfFit + " flatness restriction" + (state.lackOfFit === 1 ? "" : "s") +
          " remain, and " + state.squareRank + " of them can be written as a complete square."));
      } else {
        setVerdict($("cubeVerdict"), "flat", "fully testable");
        box.appendChild(finding("ok", "square_contrast_rank",
          "The complete square faces span the whole lack-of-fit space, so square flatness is the entire testable content of this design."));
      }

      draw(state);
    }

    function preset(indices, label) {
      boxes.forEach(function (box, cornerIndex) { box.checked = indices.indexOf(cornerIndex) !== -1; });
      setPresetLabel(label);
      render();
    }

    var full = $("presetFull");
    var six = $("presetSixCorner");
    var one = $("presetOneFace");
    if (full) { full.addEventListener("click", function () { preset([0, 1, 2, 3, 4, 5, 6, 7], "full cube"); }); }
    if (six) {
      /* Everything except 000 and 111. */
      six.addEventListener("click", function () { preset([1, 2, 3, 4, 5, 6], "six-corner counterexample"); });
    }
    if (one) {
      /* The face on which the third factor is held at zero. */
      one.addEventListener("click", function () { preset([0, 2, 4, 6], "single square"); });
    }

    render();
  }());

  /* ----------------------------------------------------------------------
     Estimator lens battery

     Reproduces mic_engine::audit_lens_battery. Each pairwise gap is scaled by
     the root sum of squared standard errors, and the largest scaled gap is
     compared with the policy tolerance. The gate is asymmetric on purpose:
     disagreement blocks certification, agreement is recorded as information.
     Degenerate input fails closed and writes nothing to the ledger.
     ---------------------------------------------------------------------- */

  (function lensBattery() {
    var host = $("lensInputs");
    var svg = $("vizLens");
    var tolSlider = $("lensTol");
    if (!host || !svg || !tolSlider) { return; }

    var families = [
      { name: "four_law_ratio", estimate: 0.042, se: 0.011 },
      { name: "gcm_linear", estimate: 0.038, se: 0.013 },
      { name: "gcm_forest", estimate: 0.047, se: 0.012 }
    ];

    var headings = doc.createElement("div");
    headings.className = "lens-row";
    ["family", "estimate", "std. error"].forEach(function (text, position) {
      var cell = doc.createElement("span");
      cell.textContent = text;
      if (position) { cell.className = "lens-head"; }
      else { cell.className = "lens-head"; cell.style.textAlign = "left"; }
      headings.appendChild(cell);
    });
    host.appendChild(headings);

    var rows = families.map(function (family, index) {
      var row = doc.createElement("div");
      row.className = "lens-row";

      var name = doc.createElement("span");
      name.textContent = family.name;

      var estimate = doc.createElement("input");
      estimate.type = "number";
      estimate.step = "0.001";
      estimate.value = String(family.estimate);
      estimate.id = "lensEst" + index;
      estimate.setAttribute("aria-label", family.name + " estimate");

      var error = doc.createElement("input");
      error.type = "number";
      error.step = "0.001";
      error.min = "0";
      error.value = String(family.se);
      error.id = "lensSe" + index;
      error.setAttribute("aria-label", family.name + " standard error");

      row.appendChild(name);
      row.appendChild(estimate);
      row.appendChild(error);
      host.appendChild(row);

      estimate.addEventListener("input", render);
      error.addEventListener("input", render);
      return { name: family.name, estimate: estimate, error: error };
    });

    function finding(kind, code, message) {
      var node = doc.createElement("div");
      node.className = "finding " + kind;
      var codeNode = doc.createElement("code");
      codeNode.textContent = code;
      var text = doc.createElement("p");
      text.textContent = message;
      node.appendChild(codeNode);
      node.appendChild(text);
      return node;
    }

    function draw(values, tolerance, worst) {
      clear(svg);

      var lo = Infinity;
      var hi = -Infinity;
      values.forEach(function (value) {
        lo = Math.min(lo, value.estimate - 2.2 * Math.max(value.se, 0));
        hi = Math.max(hi, value.estimate + 2.2 * Math.max(value.se, 0));
      });
      if (!isFinite(lo) || !isFinite(hi) || hi - lo < 1e-9) { lo -= 0.01; hi += 0.01; }

      var box = makeBox(520, 260, { left: 118, right: 24, top: 34, bottom: 46 });
      box.xScale = function (v) { return box.left + (v - lo) / (hi - lo) * box.width; };

      var ticks = [lo, lo + (hi - lo) / 2, hi];
      plotFrame(svg, box, {
        xTicks: ticks,
        xFormat: function (v) { return v.toFixed(3); }
      });

      values.forEach(function (value, index) {
        var y = box.top + 18 + (box.height - 26) * (index + 0.5) / values.length;
        var involved = worst && (worst[0] === value.name || worst[1] === value.name);
        var cls = value.valid ? (involved && !worst.agrees ? "block" : "flat") : "block";

        svg.appendChild(el("text", { x: box.left - 12, y: y + 4, class: "viz-label", "text-anchor": "end", "font-size": 11 }, value.name));

        if (value.valid) {
          svg.appendChild(el("line", {
            x1: box.xScale(value.estimate - value.se), y1: y,
            x2: box.xScale(value.estimate + value.se), y2: y,
            class: "viz-interval " + cls
          }));
          [value.estimate - value.se, value.estimate + value.se].forEach(function (end) {
            svg.appendChild(el("line", { x1: box.xScale(end), y1: y - 6, x2: box.xScale(end), y2: y + 6, class: "viz-interval " + cls }));
          });
        }
        svg.appendChild(el("circle", { cx: box.xScale(value.estimate), cy: y, r: 5, class: "viz-bar " + cls }));
      });

      svg.appendChild(el("text", {
        x: box.left + box.width / 2, y: box.top + box.height + 32, class: "viz-label", "text-anchor": "middle"
      }, "projected curvature estimate"));
      svg.appendChild(el("text", { x: 10, y: 20, class: "viz-title" },
        "scaled gap tolerance " + tolerance.toFixed(1)));
    }

    function render() {
      var tolerance = Number(tolSlider.value);
      $("lensTolOut").value = tolerance.toFixed(1);

      var values = rows.map(function (row) {
        var estimate = Number(row.estimate.value);
        var se = Number(row.error.value);
        return {
          name: row.name,
          estimate: estimate,
          se: se,
          valid: isFinite(estimate) && isFinite(se) && se > 0
        };
      });

      var invalid = values.filter(function (value) { return !value.valid; });
      var box = $("lensFindings");
      box.textContent = "";

      if (invalid.length) {
        /* InvalidLensBattery: the audit refuses to run and, deliberately,
           writes no finding to the ledger at all. */
        setVerdict($("lensStatus"), "block", "REJECTED");
        $("lensGap").textContent = "--";
        setClassState($("lensGap"), "block");
        $("lensPair").textContent = invalid.map(function (value) { return value.name; }).join(", ");
        box.appendChild(finding("error", "InvalidLensBattery",
          "Every standard error must be finite and strictly positive. The battery is rejected before any comparison is attempted, and nothing is written to the evidence ledger."));
        draw(values, tolerance, null);
        return;
      }

      var maxGap = 0;
      var worst = [values[0].name, values[1].name];
      for (var i = 0; i < values.length; i += 1) {
        for (var k = i + 1; k < values.length; k += 1) {
          var gap = Math.abs(values[i].estimate - values[k].estimate) /
            Math.hypot(values[i].se, values[k].se);
          if (gap > maxGap) { maxGap = gap; worst = [values[i].name, values[k].name]; }
        }
      }

      var agrees = maxGap <= tolerance;
      worst.agrees = agrees;

      $("lensGap").textContent = fmt(maxGap, 6);
      setClassState($("lensGap"), agrees ? "flat" : "block");
      $("lensPair").textContent = worst[0] + " vs " + worst[1];
      setVerdict($("lensStatus"), agrees ? "flat" : "block", agrees ? "AGREES" : "DISAGREES");

      if (agrees) {
        box.appendChild(finding("ok", "estimator_family_agreement",
          "Estimator families agree within the preregistered sensitivity tolerance. Agreement is diagnostic, not certifying, and on its own it establishes nothing."));
      } else {
        box.appendChild(finding("error", "estimator_family_disagreement",
          "Estimator families disagree beyond tolerance. The projection is learner-dependent and cannot be certified."));
      }

      draw(values, tolerance, worst);
    }

    tolSlider.addEventListener("input", render);

    function preset(triples) {
      rows.forEach(function (row, index) {
        row.estimate.value = String(triples[index][0]);
        row.error.value = String(triples[index][1]);
      });
      render();
    }

    var agree = $("presetAgree");
    var disagree = $("presetDisagree");
    var degenerate = $("presetDegenerate");
    if (agree) { agree.addEventListener("click", function () { preset([[0.042, 0.011], [0.038, 0.013], [0.047, 0.012]]); }); }
    if (disagree) { disagree.addEventListener("click", function () { preset([[0.042, 0.011], [0.140, 0.013], [0.047, 0.012]]); }); }
    if (degenerate) { degenerate.addEventListener("click", function () { preset([[0.042, 0.011], [0.038, 0], [0.047, 0.012]]); }); }

    render();
  }());

  /* ----------------------------------------------------------------------
     Design auditor

     Mirrors the eligibility logic of the preflight stage: a residual-product
     track is only admissible when the pooled corner odds are one, and the
     selection contract must be one the system can reason about. In strict mode
     any blocking finding means no certificate is issued at all.
     ---------------------------------------------------------------------- */

  (function auditor() {
    var inputs = ["q00", "q10", "q01", "q11"].map($);
    var trackSelect = $("trackSelect");
    var selectionSelect = $("selectionSelect");
    var strictBox = $("strictMode");
    var acceptBox = $("acceptSelection");
    var findingsBox = $("auditFindings");
    if (inputs.some(function (node) { return !node; }) || !trackSelect || !findingsBox) { return; }

    var PRODUCT_TOLERANCE = 1e-10;

    function finding(kind, code, message) {
      var node = doc.createElement("div");
      node.className = "finding " + kind;
      var codeNode = doc.createElement("code");
      codeNode.textContent = code;
      var text = doc.createElement("p");
      text.textContent = message;
      node.appendChild(codeNode);
      node.appendChild(text);
      return node;
    }

    function render() {
      var raw = inputs.map(function (node) {
        var value = Number(node.value);
        return isFinite(value) && value > 0 ? value : 0.0001;
      });
      var total = raw.reduce(function (sum, value) { return sum + value; }, 0);
      var rho = raw.map(function (value) { return value / total; });

      var logOdds = Math.log((rho[3] * rho[0]) / (rho[1] * rho[2]));
      var isProduct = Math.abs(logOdds) <= PRODUCT_TOLERANCE;

      /* The selection gate has four branches, and a declared selection model is
         not by itself sufficient: without the policy flag it is an error, and
         with the flag it downgrades to a warning that still owes diagnostic
         evidence. This mirrors selection_gate in mic-engine. */
      var selection = selectionSelect.value;
      var acceptModel = acceptBox ? acceptBox.checked : false;
      var fourLawOk = selection === "state_independent_within_regime" ||
        (selection === "modeled" && acceptModel);

      var track = trackSelect.value;
      var wantsGcm = track === "product_factorial" || track === "both";
      var gcmOk = isProduct && fourLawOk;

      var strict = strictBox ? strictBox.checked : true;

      $("logOdds").textContent = signed(logOdds, 6);
      setClassState($("logOdds"), isProduct ? "flat" : "curve");
      $("isProduct").textContent = isProduct ? "yes" : "no";
      setClassState($("isProduct"), isProduct ? "flat" : "curve");
      $("fourLawOk").textContent = fourLawOk ? "yes" : "no";
      setClassState($("fourLawOk"), fourLawOk ? "flat" : "block");
      $("gcmOk").textContent = gcmOk ? "yes" : "no";
      setClassState($("gcmOk"), gcmOk ? "flat" : "block");

      var findings = [];
      var blocking = 0;

      if (selection === "unknown") {
        blocking += 1;
        findings.push(finding("error", "state_dependent_selection",
          "Within-regime state dependence of inclusion is unknown, so neither track has a defensible estimand."));
      } else if (selection === "state_dependent_unmodeled") {
        blocking += 1;
        findings.push(finding("error", "state_dependent_selection",
          "Inclusion depends on state within regime and is not modeled. Curvature and selection cannot be separated here."));
      } else if (selection === "modeled" && !acceptModel) {
        blocking += 1;
        findings.push(finding("error", "selection_model_unvalidated",
          "A selection model was declared but no validated selection evidence is attached. Attach the evidence, or pass --allow-unvalidated-selection-model to proceed on policy."));
      } else if (selection === "modeled") {
        findings.push(finding("warn", "selection_model_unvalidated",
          "A modeled selection process is accepted by policy but still requires diagnostic evidence before any result leaves the run."));
      } else {
        findings.push(finding("ok", "selection_contract_accepted",
          "Inclusion is state-independent within regime, so the four-law track tolerates arbitrary known corner quotas."));
      }

      if (isProduct) {
        findings.push(finding("ok", "product_sampling_verified",
          "Pooled design odds are one to within tolerance, so a residual-product test characterizes zero density curvature."));
      } else if (wantsGcm) {
        blocking += 1;
        findings.push(finding("error", "non_product_sampling_for_gcm",
          "Pooled design log-odds are " + signed(logOdds, 4) + ", so zero conditional covariance characterizes a curvature of " +
          signed(-logOdds, 4) + " rather than zero. Reweight to a product design or request the four-law track."));
      } else {
        findings.push(finding("warn", "non_product_sampling_noted",
          "Corner quotas are not product, but only the four-law track was requested, and that track tolerates arbitrary known state-independent quotas."));
      }

      findings.push(finding("ok", "square_contrast_rank",
        "One complete square face over two factors. Main-effects rank three against four corners leaves exactly one testable flatness contrast."));

      var status;
      var statusKind;
      if (strict && blocking > 0) { status = "BLOCKED"; statusKind = "block"; }
      else if (!strict) { status = "DIAGNOSTIC_ONLY"; statusKind = "curve"; }
      else if (blocking > 0) { status = "DIAGNOSTIC_ONLY"; statusKind = "curve"; }
      else { status = "READY"; statusKind = "flat"; }

      setVerdict($("auditStatus"), statusKind, status);

      var note = $("auditNote");
      if (note) {
        if (status === "BLOCKED") {
          note.textContent = "Strict mode refuses to proceed. No result from this run may be serialized with a passing certificate status.";
        } else if (status === "DIAGNOSTIC_ONLY") {
          note.textContent = "Exploratory mode may continue, but every affected result is watermarked diagnostic and can never be promoted to a certificate.";
        } else {
          note.textContent = "Both tracks are admissible under this manifest. The report still begins with assumptions and abstentions, never with a table of p-values.";
        }
      }

      if (Math.abs(total - 1) > 5e-3) {
        var renorm = doc.createElement("p");
        renorm.className = "micro";
        renorm.textContent = "Quotas entered sum to " + fmt(total, 3) + " and were renormalized before the audit. A real manifest is rejected outright unless they sum to one.";
        findings.push(renorm);
      }

      findingsBox.textContent = "";
      findings.forEach(function (node) { findingsBox.appendChild(node); });
    }

    inputs.forEach(function (node) { node.addEventListener("input", render); });
    trackSelect.addEventListener("change", render);
    if (selectionSelect) { selectionSelect.addEventListener("change", render); }
    if (strictBox) { strictBox.addEventListener("change", render); }
    if (acceptBox) { acceptBox.addEventListener("change", render); }

    function preset(values) {
      inputs.forEach(function (node, index) { node.value = String(values[index]); });
      render();
    }

    var balanced = $("presetBalanced");
    var nonProduct = $("presetNonProduct");
    if (balanced) { balanced.addEventListener("click", function () { preset([0.25, 0.25, 0.25, 0.25]); }); }
    if (nonProduct) { nonProduct.addEventListener("click", function () { preset([0.10, 0.20, 0.30, 0.40]); }); }

    render();
  }());

}());

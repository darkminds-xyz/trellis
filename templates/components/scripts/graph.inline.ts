import { registerEscapeHandler, removeAllChildren } from "./util";
import { getFullSlug, resolveRelative, simplifySlug } from "../../util/path";

declare const fetchData: Promise<Record<string, ContentDetails>> | undefined;

type ContentDetails = {
  slug: string;
  filePath: string;
  title?: string;
  links?: string[];
  tags?: string[];
};

const loadDeps = (() => {
  let cache: Promise<[any, any, any]> | null = null;
  return () => {
    if (!cache) {
      cache = Promise.all([
        import("https://cdn.jsdelivr.net/npm/d3@7.9.0/+esm"),
        // Use Pixi v7 WebGL-first build for stability
        import("https://cdn.jsdelivr.net/npm/pixi.js@7.4.2/+esm"),
        import(
          "https://cdn.jsdelivr.net/npm/@tweenjs/tween.js@25.0.0/dist/tween.esm.js"
        ),
      ]);
    }
    return cache;
  };
})();

const registerCleanup: (fn: () => void) => void =
  typeof window !== "undefined" && typeof window.addCleanup === "function"
    ? window.addCleanup
    : () => {};

const localStorageKey = "graph-visited";
function getVisited(): Set<string> {
  return new Set(JSON.parse(localStorage.getItem(localStorageKey) ?? "[]"));
}

function addToVisited(slug: string) {
  const visited = getVisited();
  visited.add(slug);
  localStorage.setItem(localStorageKey, JSON.stringify([...visited]));
}

type TweenNode = {
  update: (time: number) => void;
  stop: () => void;
};

async function renderGraph(graph: HTMLElement, fullSlug: string) {
  const [
    {
      forceSimulation,
      forceManyBody,
      forceCenter,
      forceLink,
      forceCollide,
      forceRadial,
      zoomIdentity,
      select,
      drag,
      zoom,
    },
    pixi,
    tween,
  ] = await loadDeps();

  const { Group: TweenGroup, Tween: Tweened } = tween;
  const { Application, Container, Graphics, Circle, Text } = pixi;

  const colorToNumber = (c: string) => {
    try {
      return new pixi.Color(c).toNumber();
    } catch {
      const hex = c.trim().startsWith("#") ? c.trim().slice(1) : c.trim();
      return parseInt(hex, 16) || 0xffffff;
    }
  };

  const slug = simplifySlug(fullSlug as any) as string;
  const visited = getVisited();
  removeAllChildren(graph);

  let {
    drag: enableDrag,
    zoom: enableZoom,
    depth,
    scale,
    repelForce,
    centerForce,
    linkDistance,
    fontSize,
    opacityScale,
    removeTags,
    showTags,
    focusOnHover,
    enableRadial,
  } = JSON.parse(graph.dataset["cfg"] ?? "{}");

  if (!fetchData) {
    return () => {};
  }

  const data: Map<string, ContentDetails> = new Map(
    Object.entries<ContentDetails>(await fetchData).map(([k, v]) => [
      simplifySlug(k as any) as string,
      v,
    ])
  );

  const links: Array<{ source: string; target: string }> = [];
  const tags: string[] = [];
  const validLinks = new Set(data.keys());

  const tweens = new Map<string, TweenNode>();
  for (const [source, details] of data.entries()) {
    const outgoing = details.links ?? [];
    for (const dest of outgoing) {
      if (validLinks.has(dest)) {
        links.push({ source: source, target: dest });
      }
    }
    if (showTags) {
      const localTags = (details.tags ?? [])
        .filter((tag) => !removeTags.includes(tag))
        .map((tag) => simplifySlug(("tags/" + tag) as any) as string);

      tags.push(...localTags.filter((tag) => !tags.includes(tag)));
      for (const tag of localTags) {
        links.push({ source: source, target: tag });
      }
    }
  }

  const neighbourhood = new Set<string>();
  const wl: Array<string> = [slug, "__SENTINEL"];
  if (depth >= 0) {
    while (depth >= 0 && wl.length > 0) {
      const cur = wl.shift()!;
      if (cur === "__SENTINEL") {
        depth--;
        wl.push("__SENTINEL");
      } else {
        neighbourhood.add(cur);
        const outgoing = links.filter((l) => l.source === cur);
        const incoming = links.filter((l) => l.target === cur);
        wl.push(
          ...outgoing.map((l) => l.target),
          ...incoming.map((l) => l.source)
        );
      }
    }
  } else {
    validLinks.forEach((id) => neighbourhood.add(id));
    if (showTags) tags.forEach((tag) => neighbourhood.add(tag));
  }

  const nodes = [...neighbourhood].map((url) => {
    const text = url.startsWith("tags/")
      ? "#" + url.substring(5)
      : data.get(url)?.title ?? url;
    return {
      id: url,
      text,
      tags: data.get(url)?.tags ?? [],
    };
  });
  const graphData = {
    nodes,
    links: links
      .filter((l) => neighbourhood.has(l.source) && neighbourhood.has(l.target))
      .map((l) => ({
        source: nodes.find((n) => n.id === l.source)!,
        target: nodes.find((n) => n.id === l.target)!,
      })),
  };

  const width = graph.offsetWidth;
  const height = Math.max(graph.offsetHeight, 250);

  const simulation = forceSimulation(graphData.nodes)
    .force("charge", forceManyBody().strength(-100 * repelForce))
    .force("center", forceCenter().strength(centerForce))
    .force("link", forceLink(graphData.links).distance(linkDistance))
    .force("collide", forceCollide((n: any) => nodeRadius(n)).iterations(3));

  const radius = (Math.min(width, height) / 2) * 0.8;
  if (enableRadial)
    simulation.force("radial", forceRadial(radius).strength(0.2));
  const cssVars = [
    "--secondary",
    "--tertiary",
    "--gray",
    "--light",
    "--lightgray",
    "--dark",
    "--darkgray",
    "--bodyFont",
  ] as const;
  const computedStyleMap = cssVars.reduce((acc, key) => {
    acc[key] = getComputedStyle(document.documentElement).getPropertyValue(key);
    return acc;
  }, {} as Record<string, string>);

  const color = (d: any) => {
    const isCurrent = d.id === slug;
    if (isCurrent) {
      return computedStyleMap["--secondary"];
    } else if (visited.has(d.id) || d.id.startsWith("tags/")) {
      return computedStyleMap["--tertiary"];
    } else {
      return computedStyleMap["--gray"];
    }
  };

  function nodeRadius(d: any) {
    const numLinks = graphData.links.filter(
      (l) => l.source.id === d.id || l.target.id === d.id
    ).length;
    return 2 + Math.sqrt(numLinks);
  }

  let hoveredNodeId: string | null = null;
  let hoveredNeighbours: Set<string> = new Set();
  const linkRenderData: any[] = [];
  const nodeRenderData: any[] = [];
  function updateHoverInfo(newHoveredId: string | null) {
    hoveredNodeId = newHoveredId;

    if (newHoveredId === null) {
      hoveredNeighbours = new Set();
      for (const n of nodeRenderData) n.active = false;
      for (const l of linkRenderData) l.active = false;
    } else {
      hoveredNeighbours = new Set();
      for (const l of linkRenderData) {
        const linkData = l.simulationData;
        if (
          linkData.source.id === newHoveredId ||
          linkData.target.id === newHoveredId
        ) {
          hoveredNeighbours.add(linkData.source.id);
          hoveredNeighbours.add(linkData.target.id);
        }

        l.active =
          linkData.source.id === newHoveredId ||
          linkData.target.id === newHoveredId;
      }

      for (const n of nodeRenderData) {
        n.active = hoveredNeighbours.has(n.simulationData.id);
      }
    }
  }

  let dragStartTime = 0;
  let dragging = false;

  function renderLinks() {
    tweens.get("link")?.stop();
    const tweenGroup = new TweenGroup();

    for (const l of linkRenderData) {
      let alpha = 1;
      if (hoveredNodeId) {
        alpha = l.active ? 1 : 0.2;
      }

      l.color = l.active
        ? computedStyleMap["--gray"]
        : computedStyleMap["--lightgray"];
      tweenGroup.add(new Tweened(l).to({ alpha }, 200));
    }

    tweenGroup.getAll().forEach((tw: any) => tw.start());
    tweens.set("link", {
      update: tweenGroup.update.bind(tweenGroup),
      stop() {
        tweenGroup.getAll().forEach((tw: any) => tw.stop());
      },
    });
  }

  function renderLabels() {
    tweens.get("label")?.stop();
    const tweenGroup = new TweenGroup();

    const defaultScale = 1 / scale;
    const activeScale = defaultScale * 1.1;
    for (const n of nodeRenderData) {
      const nodeId = n.simulationData.id;

      if (hoveredNodeId === nodeId) {
        tweenGroup.add(
          new Tweened(n.label).to(
            {
              alpha: 1,
              scale: { x: activeScale, y: activeScale },
            },
            100
          )
        );
      } else {
        tweenGroup.add(
          new Tweened(n.label).to(
            {
              alpha: n.label.alpha,
              scale: { x: defaultScale, y: defaultScale },
            },
            100
          )
        );
      }
    }

    tweenGroup.getAll().forEach((tw: any) => tw.start());
    tweens.set("label", {
      update: tweenGroup.update.bind(tweenGroup),
      stop() {
        tweenGroup.getAll().forEach((tw: any) => tw.stop());
      },
    });
  }

  function renderNodes() {
    tweens.get("hover")?.stop();

    const tweenGroup = new TweenGroup();
    for (const n of nodeRenderData) {
      let alpha = 1;
      if (hoveredNodeId !== null && focusOnHover) {
        alpha = n.active ? 1 : 0.2;
      }

      tweenGroup.add(new Tweened(n.gfx, tweenGroup).to({ alpha }, 200));
    }

    tweenGroup.getAll().forEach((tw: any) => tw.start());
    tweens.set("hover", {
      update: tweenGroup.update.bind(tweenGroup),
      stop() {
        tweenGroup.getAll().forEach((tw: any) => tw.stop());
      },
    });
  }

  function renderPixiFromD3() {
    renderNodes();
    renderLinks();
    renderLabels();
  }

  tweens.forEach((tween) => tween.stop());
  tweens.clear();

  // Pixi v7: instantiate with options; app.view is the canvas to append.
  const app = new Application({
    width,
    height,
    antialias: true,
    autoStart: false,
    autoDensity: true,
    backgroundAlpha: 0,
    resolution: window.devicePixelRatio,
  });
  let animFrameId = 0;

  if (!app.view) {
    return () => {};
  }
  graph.appendChild(app.view as HTMLCanvasElement);

  const stage = app.stage;
  stage.interactive = false;

  // Simple z-index ordering; render groups not used in v7.
  stage.sortableChildren = true;
  const labelsContainer = new Container();
  labelsContainer.zIndex = 3;
  const nodesContainer = new Container();
  nodesContainer.zIndex = 2;
  const linkContainer = new Container();
  linkContainer.zIndex = 1;
  stage.addChild(nodesContainer, labelsContainer, linkContainer);

  for (const n of graphData.nodes) {
    const nodeId = n.id as string;

    const label = new Text(n.text, {
      fontSize: fontSize * 15,
      fill: computedStyleMap["--dark"],
      fontFamily: computedStyleMap["--bodyFont"],
    } as any);
    label.alpha = 0;
    if ((label as any).anchor?.set) {
      (label as any).anchor.set(0.5, 1.2);
    }
    label.scale.set(1 / scale);
    (label as any).resolution = window.devicePixelRatio * 4;

    let oldLabelOpacity = 0;
    const isTagNode = nodeId.startsWith("tags/");
    const gfx: any = new Graphics();
    gfx.eventMode = "static";
    gfx.cursor = "pointer";
    gfx.label = nodeId;
    gfx.hitArea = new Circle(0, 0, nodeRadius(n));
    const fillColor = colorToNumber(
      isTagNode ? computedStyleMap["--light"] : color(n)
    );
    gfx.beginFill(fillColor);
    gfx.drawCircle(0, 0, nodeRadius(n));
    gfx.endFill();
    gfx.on("pointerover", (e: any) => {
      updateHoverInfo(e.currentTarget.label);
      oldLabelOpacity = label.alpha;
      if (!dragging) {
        renderPixiFromD3();
      }
    });
    gfx.on("pointerleave", () => {
      updateHoverInfo(null);
      label.alpha = oldLabelOpacity;
      if (!dragging) {
        renderPixiFromD3();
      }
    });

    if (isTagNode) {
      const strokeColor = colorToNumber(computedStyleMap["--tertiary"]);
      gfx.lineStyle(2, strokeColor, 1);
      gfx.drawCircle(0, 0, nodeRadius(n));
    }

    nodesContainer.addChild(gfx);
    labelsContainer.addChild(label);

    nodeRenderData.push({
      simulationData: n,
      gfx,
      label,
      color: color(n),
      alpha: 1,
      active: false,
    });
  }

  for (const l of graphData.links) {
    const gfx = new Graphics();
    gfx.eventMode = "none";
    linkContainer.addChild(gfx);

    linkRenderData.push({
      simulationData: l,
      gfx,
      color: computedStyleMap["--lightgray"],
      alpha: 1,
      active: false,
    });
  }

  let currentTransform = zoomIdentity;
  if (enableDrag) {
    select(app.view as HTMLCanvasElement).call(
      drag()
        .container(() => app.view)
        .subject(
          () =>
            graphData.nodes.find(
              (n: any) => (n as any).id === hoveredNodeId
            ) as any
        )
        .on("start", function dragstarted(event: any) {
          if (!event.active) simulation.alphaTarget(1).restart();
          event.subject.fx = event.subject.x;
          event.subject.fy = event.subject.y;
          event.subject.__initialDragPos = {
            x: event.subject.x,
            y: event.subject.y,
            fx: event.subject.fx,
            fy: event.subject.fy,
          };
          dragStartTime = Date.now();
          dragging = true;
        })
        .on("drag", function dragged(event: any) {
          const initPos = event.subject.__initialDragPos;
          event.subject.fx =
            initPos.x + (event.x - initPos.x) / currentTransform.k;
          event.subject.fy =
            initPos.y + (event.y - initPos.y) / currentTransform.k;
        })
        .on("end", function dragended(event: any) {
          if (!event.active) simulation.alphaTarget(0);
          event.subject.fx = null;
          event.subject.fy = null;
          dragging = false;

          if (Date.now() - dragStartTime < 500) {
            const node = graphData.nodes.find(
              (n: any) => (n as any).id === event.subject.id
            ) as any;
            const targ = resolveRelative(fullSlug as any, node.id);
            if (typeof (window as any).spaNavigate === "function") {
              (window as any).spaNavigate(
                new URL(targ, window.location.toString())
              );
            } else {
              window.location.assign(targ as any);
            }
          }
        })
    );
  } else {
    for (const node of nodeRenderData) {
      node.gfx.on("click", () => {
        const targ = resolveRelative(fullSlug as any, node.simulationData.id);
        if (typeof (window as any).spaNavigate === "function") {
          (window as any).spaNavigate(
            new URL(targ, window.location.toString())
          );
        } else {
          window.location.assign(targ as any);
        }
      });
    }
  }

  if (enableZoom) {
    select(app.view as HTMLCanvasElement).call(
      zoom()
        .extent([
          [0, 0],
          [width, height],
        ])
        .scaleExtent([0.25, 4])
        .on("zoom", ({ transform }: any) => {
          currentTransform = transform;
          stage.scale.set(transform.k, transform.k);
          stage.position.set(transform.x, transform.y);

          const scaleAmount = transform.k * opacityScale;
          let scaleOpacity = Math.max((scaleAmount - 1) / 3.75, 0);
          const activeNodes = nodeRenderData
            .filter((n) => n.active)
            .flatMap((n) => n.label);

          for (const label of labelsContainer.children) {
            if (!activeNodes.includes(label)) {
              (label as any).alpha = scaleOpacity;
            }
          }
        })
    );
  }

  let stopAnimation = false;
  function animate(time: number) {
    if (stopAnimation) return;
    for (const n of nodeRenderData) {
      const { x, y } = n.simulationData as any;
      if (x === undefined || y === undefined) continue;
      n.gfx.position.set(x + width / 2, y + height / 2);
      if (n.label) {
        n.label.position.set(x + width / 2, y + height / 2);
      }
    }

    for (const l of linkRenderData) {
      const linkData = l.simulationData as any;
      l.gfx.clear();
      l.gfx.lineStyle(1, colorToNumber(l.color), l.alpha);
      l.gfx.moveTo(
        linkData.source.x + width / 2,
        linkData.source.y + height / 2
      );
      l.gfx.lineTo(
        linkData.target.x + width / 2,
        linkData.target.y + height / 2
      );
    }

    tweens.forEach((t) => t.update(time));
    try {
      app.renderer.render(stage);
    } catch (err) {
      stopAnimation = true;
      console.error("Graph render failed; stopping animation", err);
      return;
    }
    animFrameId = requestAnimationFrame(animate);
  }

  animFrameId = requestAnimationFrame(animate);
  return () => {
    stopAnimation = true;
    cancelAnimationFrame(animFrameId);
    // no canvas event listeners to clean up in v7 path
    app.destroy(true);
  };
}

let localGraphCleanups: Array<() => void> = [];
let globalGraphCleanups: Array<() => void> = [];

function cleanupLocalGraphs() {
  for (const cleanup of localGraphCleanups) cleanup();
  localGraphCleanups = [];
}

function cleanupGlobalGraphs() {
  for (const cleanup of globalGraphCleanups) cleanup();
  globalGraphCleanups = [];
}

async function renderLocalGraphs(slug: string) {
  cleanupLocalGraphs();
  const localGraphContainers =
    document.getElementsByClassName("graph-container");
  for (const container of Array.from(localGraphContainers)) {
    const cleanup = await renderGraph(container as HTMLElement, slug);
    localGraphCleanups.push(cleanup);
  }
}

async function renderGlobalGraph(slug: string) {
  const containers = Array.from(
    document.getElementsByClassName("global-graph-outer")
  ) as HTMLElement[];
  for (const container of containers) {
    if (!container.classList.contains("active")) {
      container.classList.add("active");
      const sidebar = container.closest(".sidebar") as HTMLElement;
      if (sidebar) sidebar.style.zIndex = "1";

      const graphContainer = container.querySelector(
        ".global-graph-container"
      ) as HTMLElement;
      registerEscapeHandler(container, hideGlobalGraph);
      if (graphContainer) {
        globalGraphCleanups.push(await renderGraph(graphContainer, slug));
      }
    }
  }
}

function hideGlobalGraph() {
  cleanupGlobalGraphs();
  const containers = Array.from(
    document.getElementsByClassName("global-graph-outer")
  ) as HTMLElement[];
  for (const container of containers) {
    container.classList.remove("active");
    const sidebar = container.closest(".sidebar") as HTMLElement;
    if (sidebar) sidebar.style.zIndex = "";
  }
}

function setupGraphShortcuts(slug: string) {
  const containerIcons = document.getElementsByClassName("global-graph-icon");
  Array.from(containerIcons).forEach((icon) => {
    const handler = () => {
      const anyOpen = Array.from(
        document.getElementsByClassName("global-graph-outer")
      ).some((c) => c.classList.contains("active"));
      anyOpen ? hideGlobalGraph() : renderGlobalGraph(slug);
    };
    icon.addEventListener("click", handler);
    registerCleanup(() => icon.removeEventListener("click", handler));
  });

  const shortcutHandler = (e: KeyboardEvent) => {
    if (e.key === "g" && (e.ctrlKey || e.metaKey) && !e.shiftKey) {
      e.preventDefault();
      const anyGlobalGraphOpen = Array.from(
        document.getElementsByClassName("global-graph-outer")
      ).some((container) => container.classList.contains("active"));
      anyGlobalGraphOpen ? hideGlobalGraph() : renderGlobalGraph(slug);
    }
  };

  document.addEventListener("keydown", shortcutHandler);
  registerCleanup(() =>
    document.removeEventListener("keydown", shortcutHandler)
  );
}

document.addEventListener("DOMContentLoaded", async () => {
  const slug = getFullSlug(window) as string;
  addToVisited(simplifySlug(slug as any) as string);

  await renderLocalGraphs(slug);
  setupGraphShortcuts(slug);

  const handleThemeChange = () => {
    void renderLocalGraphs(slug);
  };
  document.addEventListener("themechange", handleThemeChange);
  registerCleanup(() =>
    document.removeEventListener("themechange", handleThemeChange)
  );
});

<script lang="ts">
    import { onMount } from "svelte";
    import type {
        TransitionGraphEdge,
        TransitionGraphNode,
    } from "../../api/commands";

    type NodePos = { x: number; y: number; r: number };

    type Props = {
        nodes: TransitionGraphNode[];
        edges: TransitionGraphEdge[];
        selectedNodeKey?: string | null;
        selectedEdgeKey?: string | null;
        onNodeSelect: (key: string | null) => void;
        onEdgeSelect: (key: string | null) => void;
    };

    let {
        nodes,
        edges,
        selectedNodeKey = null,
        selectedEdgeKey = null,
        onNodeSelect,
        onEdgeSelect,
    }: Props = $props();

    let nodePositions: Record<string, NodePos> = $state({});
    let scale = $state(1);
    let translateX = $state(0);
    let translateY = $state(0);
    let isPanning = $state(false);
    let isWheelInteracting = $state(false);
    let hoveredNodeKey: string | null = $state(null);
    let hoveredEdgeKey: string | null = $state(null);
    let svgEl: SVGSVGElement | undefined = $state(undefined);
    let svgWidth = $state(800);
    let svgHeight = $state(600);
    let panStartX = 0;
    let panStartY = 0;
    let pointerDownX = 0;
    let pointerDownY = 0;
    let didDrag = $state(false);
    let animationKey = $state(0);
    let wheelTimer: ReturnType<typeof setTimeout> | undefined;

    const activeInteraction = $derived(
        Boolean(
            hoveredNodeKey ||
                selectedNodeKey ||
                hoveredEdgeKey ||
                selectedEdgeKey,
        ),
    );

    const edgeByKey = $derived(new Map(edges.map((edge) => [edge.key, edge])));

    $effect(() => {
        calculateLayout(nodes);
        animationKey += 1;
        requestAnimationFrame(() => reset());
    });

    onMount(() => {
        const updateSize = () => {
            if (!svgEl) return;
            const rect = svgEl.getBoundingClientRect();
            svgWidth = rect.width || 800;
            svgHeight = rect.height || 600;
        };

        const resizeObserver = new ResizeObserver(updateSize);
        if (svgEl) resizeObserver.observe(svgEl);
        updateSize();
        reset();

        if (svgEl) {
            svgEl.addEventListener("wheel", handleWheel, { passive: false });
            svgEl.addEventListener("pointerdown", handlePointerDown);
            svgEl.addEventListener("click", handleSvgClick);
            svgEl.addEventListener("pointerover", handlePointerOver);
            svgEl.addEventListener("pointerout", handlePointerOut);
        }

        window.addEventListener("pointermove", handleWindowPointerMove);
        window.addEventListener("pointerup", handleWindowPointerUp);
        window.addEventListener("pointercancel", handleWindowPointerUp);

        return () => {
            resizeObserver.disconnect();
            if (svgEl) {
                svgEl.removeEventListener("wheel", handleWheel);
                svgEl.removeEventListener("pointerdown", handlePointerDown);
                svgEl.removeEventListener("click", handleSvgClick);
                svgEl.removeEventListener("pointerover", handlePointerOver);
                svgEl.removeEventListener("pointerout", handlePointerOut);
            }
            window.removeEventListener("pointermove", handleWindowPointerMove);
            window.removeEventListener("pointerup", handleWindowPointerUp);
            window.removeEventListener("pointercancel", handleWindowPointerUp);
            if (wheelTimer) clearTimeout(wheelTimer);
        };
    });

    // composedPath を辿って data-graph-role を持つ要素を探す
    function findGraphTarget(event: Event): Element | null {
        for (const item of event.composedPath()) {
            if (item instanceof Element && item.getAttribute("data-graph-role")) {
                return item;
            }
        }
        return null;
    }

    function calculateLayout(nextNodes: TransitionGraphNode[]) {
        if (nextNodes.length === 0) {
            nodePositions = {};
            return;
        }

        const minVisits = Math.min(...nextNodes.map((node) => node.visit_count));
        const maxVisits = Math.max(...nextNodes.map((node) => node.visit_count));
        const radiusFor = (visitCount: number) => {
            if (minVisits === maxVisits) return 18;
            return (
                12 +
                ((visitCount - minVisits) / (maxVisits - minVisits)) *
                    (36 - 12)
            );
        };

        const sortedNodes = [...nextNodes].sort(
            (a, b) =>
                b.visit_count * 3 +
                b.degree -
                (a.visit_count * 3 + a.degree),
        );
        const centerNode = sortedNodes[0];
        const nextPositions: Record<string, NodePos> = {
            [centerNode.key]: {
                x: 0,
                y: 0,
                r: radiusFor(centerNode.visit_count),
            },
        };

        const outerNodes = sortedNodes.slice(1);
        const ringRadius = Math.max(200, outerNodes.length * 25);
        outerNodes.forEach((node, index) => {
            const angle = (Math.PI * 2 * index) / Math.max(1, outerNodes.length);
            nextPositions[node.key] = {
                x: Math.cos(angle) * ringRadius,
                y: Math.sin(angle) * ringRadius,
                r: radiusFor(node.visit_count),
            };
        });

        nodePositions = nextPositions;
    }

    function edgeWidth(edge: TransitionGraphEdge) {
        if (edges.length === 0) return 1.5;
        const minCount = Math.min(
            ...edges.map((item) => item.transition_count),
        );
        const maxCount = Math.max(
            ...edges.map((item) => item.transition_count),
        );
        if (minCount === maxCount) return 2.5;
        return (
            1.5 +
            ((edge.transition_count - minCount) / (maxCount - minCount)) *
                (6 - 1.5)
        );
    }

    function isEdgeHighlighted(edgeKey: string) {
        const edge = edgeByKey.get(edgeKey);
        if (!edge) return false;
        return (
            hoveredEdgeKey === edgeKey ||
            selectedEdgeKey === edgeKey ||
            (!!hoveredNodeKey &&
                (edge.from === hoveredNodeKey || edge.to === hoveredNodeKey)) ||
            (!!selectedNodeKey &&
                (edge.from === selectedNodeKey || edge.to === selectedNodeKey))
        );
    }

    function getEdgeOpacity(edgeKey: string) {
        if (activeInteraction) {
            return isEdgeHighlighted(edgeKey) ? 1 : 0.2;
        }
        return 0.6;
    }

    function isNodeHighlighted(nodeKey: string) {
        const hoveredEdge = hoveredEdgeKey ? edgeByKey.get(hoveredEdgeKey) : null;
        const selectedEdge = selectedEdgeKey
            ? edgeByKey.get(selectedEdgeKey)
            : null;
        return (
            hoveredNodeKey === nodeKey ||
            selectedNodeKey === nodeKey ||
            (!!hoveredEdge &&
                (hoveredEdge.from === nodeKey || hoveredEdge.to === nodeKey)) ||
            (!!selectedEdge &&
                (selectedEdge.from === nodeKey || selectedEdge.to === nodeKey))
        );
    }

    function getNodeOpacity(nodeKey: string) {
        if (activeInteraction) {
            return isNodeHighlighted(nodeKey) ? 1 : 0.3;
        }
        return 0.9;
    }

    function selectNode(key: string) {
        if (selectedNodeKey === key) {
            onNodeSelect(null);
        } else {
            onNodeSelect(key);
            onEdgeSelect(null);
        }
    }

    function selectEdge(key: string) {
        if (selectedEdgeKey === key) {
            onEdgeSelect(null);
        } else {
            onEdgeSelect(key);
            onNodeSelect(null);
        }
    }

    function handlePointerDown(event: PointerEvent) {
        const graphTarget = findGraphTarget(event);
        const role = graphTarget?.getAttribute("data-graph-role");

        if (role === "node" || role === "edge") return;

        isPanning = true;
        didDrag = false;
        pointerDownX = event.clientX;
        pointerDownY = event.clientY;
        panStartX = event.clientX - translateX;
        panStartY = event.clientY - translateY;

        try {
            svgEl?.setPointerCapture(event.pointerId);
        } catch {
            // setPointerCapture が使えない環境でも落とさない
        }
    }

    function handleWindowPointerMove(event: PointerEvent) {
        if (!isPanning) return;

        const dx = event.clientX - pointerDownX;
        const dy = event.clientY - pointerDownY;
        if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
            didDrag = true;
        }

        translateX = event.clientX - panStartX;
        translateY = event.clientY - panStartY;
    }

    function handleWindowPointerUp(event: PointerEvent) {
        isPanning = false;

        try {
            if (svgEl?.hasPointerCapture(event.pointerId)) {
                svgEl.releasePointerCapture(event.pointerId);
            }
        } catch {
            // releasePointerCapture が失敗しても落とさない
        }
    }

    function handleWheel(event: WheelEvent) {
        event.preventDefault();
        event.stopPropagation();

        isWheelInteracting = true;
        if (wheelTimer) clearTimeout(wheelTimer);
        wheelTimer = setTimeout(() => {
            isWheelInteracting = false;
        }, 200);

        const delta = event.deltaY > 0 ? 1 / 1.1 : 1.1;
        const newScale = Math.min(3, Math.max(0.35, scale * delta));
        const rect = svgEl?.getBoundingClientRect();
        if (!rect) return;

        const mouseX = event.clientX - rect.left;
        const mouseY = event.clientY - rect.top;
        translateX = mouseX - (mouseX - translateX) * (newScale / scale);
        translateY = mouseY - (mouseY - translateY) * (newScale / scale);
        scale = newScale;
    }

    function handleSvgClick(event: MouseEvent) {
        if (didDrag) {
            didDrag = false;
            return;
        }

        const graphTarget = findGraphTarget(event);
        const role = graphTarget?.getAttribute("data-graph-role");

        if (role === "node") {
            const key = graphTarget?.getAttribute("data-node-key");
            if (key) {
                selectNode(key);
                return;
            }
        }

        if (role === "edge") {
            const key = graphTarget?.getAttribute("data-edge-key");
            if (key) {
                selectEdge(key);
                return;
            }
        }

        onNodeSelect(null);
        onEdgeSelect(null);
    }

    function handlePointerOver(event: PointerEvent) {
        const graphTarget = findGraphTarget(event);
        const role = graphTarget?.getAttribute("data-graph-role");

        if (role === "node") {
            hoveredNodeKey = graphTarget?.getAttribute("data-node-key") ?? null;
            hoveredEdgeKey = null;
            return;
        }

        if (role === "edge") {
            hoveredEdgeKey = graphTarget?.getAttribute("data-edge-key") ?? null;
            hoveredNodeKey = null;
            return;
        }
    }

    function handlePointerOut(event: PointerEvent) {
        const graphTarget = findGraphTarget(event);
        const role = graphTarget?.getAttribute("data-graph-role");

        if (role === "node") {
            hoveredNodeKey = null;
        } else if (role === "edge") {
            hoveredEdgeKey = null;
        }
    }

    export function reset() {
        scale = 1;
        translateX = svgWidth / 2;
        translateY = svgHeight / 2;
    }

    export function fitAll() {
        if (Object.keys(nodePositions).length === 0) return;
        const xs = Object.values(nodePositions).map((p) => p.x);
        const ys = Object.values(nodePositions).map((p) => p.y);
        const minX = Math.min(...xs) - 60;
        const maxX = Math.max(...xs) + 60;
        const minY = Math.min(...ys) - 60;
        const maxY = Math.max(...ys) + 60;
        const fitScale = Math.min(
            svgWidth / (maxX - minX),
            svgHeight / (maxY - minY),
            3,
        );
        const clampedScale = Math.max(0.35, fitScale);
        const cx = (minX + maxX) / 2;
        const cy = (minY + maxY) / 2;
        translateX = svgWidth / 2 - cx * clampedScale;
        translateY = svgHeight / 2 - cy * clampedScale;
        scale = clampedScale;
    }
</script>

<div class="relative h-full w-full min-h-0">
    <svg
        bind:this={svgEl}
        class="block h-full w-full select-none"
        pointer-events="all"
        style="cursor: {isPanning ? 'grabbing' : 'grab'}; overflow: hidden; touch-action: none; overscroll-behavior: contain;"
    >
        <style>
            @keyframes fadeIn {
                from {
                    opacity: 0;
                }
                to {
                    opacity: 1;
                }
            }
        </style>
        <rect
            width="100%"
            height="100%"
            fill="rgba(0,0,0,0.001)"
            pointer-events="all"
            data-graph-role="background"
        />
        <g
            transform="translate({translateX},{translateY}) scale({scale})"
            style:transition={isPanning || isWheelInteracting
                ? "none"
                : "transform 0.2s ease-out"}
        >
            {#each edges as edge (edge.key)}
                {@const fromPos = nodePositions[edge.from]}
                {@const toPos = nodePositions[edge.to]}
                {#if fromPos && toPos}
                    <!-- hit area: 細いエッジでもクリック・hover しやすくする -->
                    <line
                        x1={fromPos.x}
                        y1={fromPos.y}
                        x2={toPos.x}
                        y2={toPos.y}
                        stroke="rgba(0,0,0,0.001)"
                        stroke-width="14"
                        stroke-linecap="round"
                        pointer-events="stroke"
                        data-graph-role="edge"
                        data-edge-key={edge.key}
                        style="cursor: pointer;"
                    />
                    <!-- 表示用ライン -->
                    <line
                        x1={fromPos.x}
                        y1={fromPos.y}
                        x2={toPos.x}
                        y2={toPos.y}
                        stroke={isEdgeHighlighted(edge.key) ? "#1e5854" : "#94a3b8"}
                        stroke-width={edgeWidth(edge)}
                        stroke-linecap="round"
                        opacity={getEdgeOpacity(edge.key)}
                        pointer-events="none"
                        class="transition-opacity duration-150"
                        style={`animation: fadeIn 0.4s ease-out both; animation-delay: ${animationKey * 0.001}s;`}
                    />
                {/if}
            {/each}

            {#each nodes as node (node.key)}
                {@const pos = nodePositions[node.key]}
                {#if pos}
                    <circle
                        cx={pos.x}
                        cy={pos.y}
                        r={pos.r}
                        fill={isNodeHighlighted(node.key) ? "#1e5854" : "#f8fafc"}
                        stroke={selectedNodeKey === node.key ? "#0f3f3b" : "#1e5854"}
                        stroke-width={selectedNodeKey === node.key ? 4 : 2}
                        opacity={getNodeOpacity(node.key)}
                        data-graph-role="node"
                        data-node-key={node.key}
                        class="transition-opacity duration-150"
                        style={`animation: fadeIn 0.4s ease-out both; animation-delay: ${animationKey * 0.001}s; cursor: pointer;`}
                    />
                    <text
                        x={pos.x}
                        y={pos.y + pos.r + 14}
                        text-anchor="middle"
                        class="select-none fill-zinc-600 text-[11px] font-bold"
                        opacity={getNodeOpacity(node.key)}
                        pointer-events="none"
                    >
                        {node.world_name.length > 16
                            ? `${node.world_name.slice(0, 16)}...`
                            : node.world_name}
                    </text>
                    <text
                        x={pos.x}
                        y={pos.y + 4}
                        text-anchor="middle"
                        class="select-none fill-white text-[10px] font-black"
                        opacity={isNodeHighlighted(node.key) ? 1 : 0}
                        pointer-events="none"
                    >
                        {node.visit_count}
                    </text>
                {/if}
            {/each}
        </g>
    </svg>
</div>

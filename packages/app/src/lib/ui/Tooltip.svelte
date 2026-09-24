<script lang="ts" module>
	// Once one tooltip has shown, the next opens at once: moving along a
	// toolbar should not wait again at every button.
	let lastHidden = 0;
</script>

<script lang="ts">
	import { autoUpdate, computePosition, flip, offset, shift, type Placement } from '@floating-ui/dom';
	import type { Snippet } from 'svelte';
	import { tooltip } from './recipes';

	/**
	 * A label shown on hover or keyboard focus. @human-kit/ui has no tooltip,
	 * so this one is ours. It describes the control (`aria-describedby`), it
	 * does not name it: an icon-only button still needs its own `aria-label`.
	 */
	type Props = { text: string; placement?: Placement; delay?: number; children: Snippet };
	let { text, placement = 'bottom', delay = 450, children }: Props = $props();

	const id = $props.id();
	let anchor: HTMLSpanElement;
	let tip: HTMLDivElement | null = $state(null);
	let open = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function show(now = false) {
		clearTimeout(timer);
		if (now || performance.now() - lastHidden < 300) open = true;
		else timer = setTimeout(() => (open = true), delay);
	}

	function hide() {
		clearTimeout(timer);
		if (open) lastHidden = performance.now();
		open = false;
	}

	function onFocusIn(e: FocusEvent) {
		if ((e.target as HTMLElement).matches(':focus-visible')) show(true);
	}

	$effect(() => {
		const target = anchor.firstElementChild;
		target?.setAttribute('aria-describedby', id);
		return () => target?.removeAttribute('aria-describedby');
	});

	$effect(() => {
		if (!open || !tip) return;
		const el = tip;
		document.body.appendChild(el);
		const onKey = (e: KeyboardEvent) => e.key === 'Escape' && hide();
		window.addEventListener('keydown', onKey);
		const stop = autoUpdate(anchor, el, () =>
			computePosition(anchor, el, {
				placement,
				strategy: 'fixed',
				middleware: [offset(6), flip(), shift({ padding: 6 })]
			}).then(({ x, y }) => {
				// left/top, not transform: the entrance animation owns transform.
				el.style.left = `${Math.round(x)}px`;
				el.style.top = `${Math.round(y)}px`;
				el.dataset.placed = 'true';
			})
		);
		return () => {
			stop();
			window.removeEventListener('keydown', onKey);
			el.remove();
		};
	});
</script>

<span
	bind:this={anchor}
	class="inline-flex"
	role="presentation"
	onpointerenter={() => show()}
	onpointerleave={hide}
	onpointerdown={hide}
	onfocusin={onFocusIn}
	onfocusout={hide}
>
	{@render children()}
</span>

{#if open}
	<div bind:this={tip} {id} role="tooltip" class={tooltip()}>{text}</div>
{:else}
	<span {id} hidden>{text}</span>
{/if}

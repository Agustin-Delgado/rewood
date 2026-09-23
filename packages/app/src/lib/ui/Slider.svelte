<script lang="ts">
	/**
	 * A range slider. @human-kit/ui has none yet, so this is the native input
	 * with the system's look; keyboard and screen readers get the native control.
	 * `onInput` fires while dragging, `onChange` once on release.
	 */
	type Props = {
		value: number;
		min: number;
		max: number;
		step?: number;
		onInput?: (value: number) => void;
		onChange?: (value: number) => void;
		disabled?: boolean;
		class?: string;
		'aria-label': string;
	};

	let { value = $bindable(), min, max, step = 1, onInput, onChange, disabled, class: className, ...rest }: Props = $props();
	const pct = $derived(max > min ? Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100)) : 0);
</script>

<input
	type="range"
	{min}
	{max}
	{step}
	{disabled}
	bind:value
	oninput={() => onInput?.(value)}
	onchange={() => onChange?.(value)}
	style="--pct: {pct}%"
	class="rw-slider {className ?? ''}"
	{...rest}
/>

<style>
	.rw-slider {
		appearance: none;
		width: 100%;
		height: 16px;
		background: transparent;
		cursor: pointer;
		margin: 0;
	}
	.rw-slider:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}
	.rw-slider::-webkit-slider-runnable-track {
		height: 4px;
		border-radius: 999px;
		background: linear-gradient(to right, var(--primary) var(--pct), var(--depth-4) var(--pct));
	}
	.rw-slider::-moz-range-track {
		height: 4px;
		border-radius: 999px;
		background: var(--depth-4);
	}
	.rw-slider::-moz-range-progress {
		height: 4px;
		border-radius: 999px;
		background: var(--primary);
	}
	.rw-slider::-webkit-slider-thumb {
		appearance: none;
		width: 14px;
		height: 14px;
		margin-top: -5px;
		border-radius: 999px;
		background: var(--depth-0);
		border: 1px solid var(--border-strong);
		box-shadow: 0 1px 3px var(--shadow-strong);
		transition: transform 0.1s;
	}
	.rw-slider::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 999px;
		background: var(--depth-0);
		border: 1px solid var(--border-strong);
		box-shadow: 0 1px 3px var(--shadow-strong);
	}
	.rw-slider:active::-webkit-slider-thumb {
		transform: scale(1.12);
	}
	.rw-slider:focus-visible {
		outline: none;
	}
	.rw-slider:focus-visible::-webkit-slider-thumb {
		box-shadow: 0 0 0 3px color-mix(in oklch, var(--ring) 35%, transparent);
	}
</style>

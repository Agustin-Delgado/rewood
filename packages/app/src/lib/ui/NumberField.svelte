<script lang="ts">
	import { NumberField } from '@human-kit/ui/numberfield';
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import { numberField } from './recipes';

	/**
	 * A number with its unit. `onChange` fires on commit (Enter, blur, a stepper
	 * or an arrow key), not on every keystroke: each change recompiles the plan.
	 */
	type Props = {
		value: number | null | undefined;
		onChange?: (value: number | null) => void;
		min?: number;
		max?: number;
		step?: number;
		unit?: string;
		steppers?: boolean;
		size?: 'xs' | 'sm' | 'md';
		disabled?: boolean;
		readonly?: boolean;
		invalid?: boolean;
		placeholder?: string;
		/** Maximum decimals shown; the value keeps its precision. */
		decimals?: number;
		id?: string;
		class?: string;
		'aria-label'?: string;
		title?: string;
	};

	let {
		value = $bindable(),
		onChange,
		min,
		max,
		step = 1,
		unit,
		steppers = false,
		size,
		decimals = 2,
		invalid = false,
		disabled,
		readonly,
		id,
		placeholder,
		title,
		class: className,
		'aria-label': ariaLabel
	}: Props = $props();

	const r = $derived(numberField({ size }));
	const formatOptions = $derived({ maximumFractionDigits: decimals, useGrouping: false });

	function change(next: number | null) {
		value = next;
		onChange?.(next);
	}
</script>

<NumberField.Root
	value={value ?? null}
	onChange={change}
	{min}
	{max}
	{step}
	{formatOptions}
	{invalid}
	{disabled}
	{readonly}
	incrementAriaLabel="Aumentar"
	decrementAriaLabel="Disminuir"
	class={r.root({ class: className })}
>
	<NumberField.Group class={r.group()} {title}>
		{#if steppers}
			<NumberField.Decrement class={r.stepper({ class: 'border-r border-border/70' })}><Minus /></NumberField.Decrement>
		{/if}
		<NumberField.Input {id} {placeholder} aria-label={ariaLabel} class={r.input({ class: unit ? 'pr-1' : '' })} />
		{#if unit}<span class={r.unit()}>{unit}</span>{/if}
		{#if steppers}
			<NumberField.Increment class={r.stepper({ class: 'border-l border-border/70' })}><Plus /></NumberField.Increment>
		{/if}
	</NumberField.Group>
</NumberField.Root>

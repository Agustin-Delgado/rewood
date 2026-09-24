import { tv, type VariantProps } from 'tailwind-variants';

/*
 * Every look in the app, in one place. Components under `src/lib/ui` apply these
 * to @human-kit/ui primitives; screens compose components and add layout only.
 * State comes from the primitives' data attributes (`data-pressed`,
 * `data-selected`, `data-focus-visible`…), never from classes toggled by hand.
 */

const focusRing = `outline-none
	data-focus-visible:ring-2 data-focus-visible:ring-ring/60 data-focus-visible:ring-offset-1 data-focus-visible:ring-offset-depth-0
	focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-1 focus-visible:ring-offset-depth-0`;

export const button = tv({
	base: `relative inline-flex shrink-0 select-none items-center justify-center gap-1.5 whitespace-nowrap
		rounded-md border font-medium transition-[background-color,border-color,color,box-shadow,opacity] duration-100 ease-out
		${focusRing}
		disabled:pointer-events-none disabled:opacity-45 data-disabled:pointer-events-none data-disabled:opacity-45
		data-pending:cursor-progress
		[&_svg]:pointer-events-none [&_svg]:shrink-0`,
	variants: {
		variant: {
			primary: `border-primary-hover/40 bg-primary text-primary-foreground raised
				hover:bg-primary-hover data-pressed:bg-primary-hover data-pressed:shadow-none`,
			secondary: `border-border-strong/70 bg-depth-0 text-foreground raised
				hover:border-border-strong hover:bg-depth-1 data-pressed:bg-depth-2 data-pressed:sunken`,
			ghost: `border-transparent text-muted-foreground
				hover:bg-depth-3/70 hover:text-foreground data-pressed:bg-depth-3`,
			soft: `border-transparent bg-primary-soft text-primary-soft-foreground
				hover:bg-primary-soft/70 data-pressed:bg-primary-soft/60`,
			destructive: `border-danger/40 bg-danger text-primary-foreground raised
				hover:bg-danger/90 data-pressed:bg-danger/85 data-pressed:shadow-none`,
			danger: 'border-transparent text-danger hover:bg-danger-soft data-pressed:bg-danger-soft',
			link: 'border-0 font-normal text-primary-soft-foreground underline-offset-3 hover:underline'
		},
		size: {
			xs: 'h-6 gap-1 px-1.5 text-xs [&_svg]:size-3.5',
			sm: 'h-7 px-2.5 text-sm [&_svg]:size-3.5',
			md: 'h-8 px-3 text-sm [&_svg]:size-4',
			'icon-xs': 'size-6 [&_svg]:size-3.5',
			'icon-sm': 'size-7 [&_svg]:size-3.5',
			icon: 'size-8 [&_svg]:size-4'
		}
	},
	compoundVariants: [{ variant: 'link', class: 'h-auto px-0' }],
	defaultVariants: { variant: 'secondary', size: 'sm' }
});
export type ButtonVariants = VariantProps<typeof button>;

/** A two-state button: toolbar switches, the pressed look of a tool. */
export const toggle = tv({
	extend: button,
	base: `data-selected:border-primary/25 data-selected:bg-primary-soft
		data-selected:text-primary-soft-foreground`,
	defaultVariants: { variant: 'ghost', size: 'sm' }
});

/** A row of mutually exclusive choices on a sunken track. */
export const segmented = tv({
	slots: {
		root: 'inline-flex max-w-full items-center gap-0.5 rounded-md border bg-depth-2 p-0.5 sunken',
		item: `inline-flex min-w-0 shrink-0 select-none items-center justify-center gap-1 whitespace-nowrap rounded-sm
			border border-transparent px-2 font-medium text-muted-foreground transition-colors duration-100
			${focusRing}
			hover:text-foreground
			data-selected:border-border data-selected:bg-depth-0 data-selected:text-foreground data-selected:raised
			data-disabled:pointer-events-none data-disabled:opacity-40 [&_svg]:size-3.5`
	},
	variants: {
		size: {
			xs: { item: 'h-5 text-2xs' },
			sm: { item: 'h-6 text-xs' },
			md: { item: 'h-7 text-sm' }
		},
		fill: { true: { root: 'flex w-full', item: 'flex-1 shrink' }, false: {} }
	},
	defaultVariants: { size: 'sm', fill: false }
});

export const tabs = tv({
	slots: {
		root: 'flex min-h-0 flex-col',
		bar: 'flex min-w-0 shrink-0 items-center',
		list: 'relative flex min-w-0 shrink-0 items-stretch overflow-x-auto',
		end: 'ml-auto flex shrink-0 items-center gap-1.5 pl-2',
		tab: `relative inline-flex select-none items-center gap-1.5 whitespace-nowrap font-medium text-muted-foreground
			transition-colors duration-100 hover:text-foreground ${focusRing}
			data-selected:text-foreground data-disabled:opacity-40 [&_svg]:size-3.5`,
		indicator: `pointer-events-none absolute left-(--active-tab-left) w-(--active-tab-width)
			transition-[left,width] duration-200 ease-out data-hidden:opacity-0`,
		panel: 'min-h-0 flex-1 outline-none'
	},
	variants: {
		variant: {
			line: {
				bar: 'border-b bg-depth-1 pr-2',
				list: 'gap-4 px-3',
				tab: 'h-9 text-sm',
				indicator: 'bottom-0 h-0.5 rounded-full bg-primary'
			},
			pill: {
				list: 'gap-0.5 rounded-md border bg-depth-2 p-0.5 sunken',
				tab: 'h-6 rounded-sm px-2 text-xs data-selected:bg-depth-0 data-selected:raised',
				indicator: 'hidden'
			}
		}
	},
	defaultVariants: { variant: 'line' }
});

const fieldSurface = `w-full min-w-0 rounded-md border border-border-strong/70 bg-depth-0 text-foreground sunken
	transition-[border-color,box-shadow] duration-100 outline-none
	placeholder:text-subtle-foreground
	hover:border-border-strong
	focus:border-ring focus:ring-2 focus:ring-ring/25
	data-invalid:border-danger data-invalid:ring-danger/20
	disabled:cursor-not-allowed disabled:opacity-50`;

export const input = tv({
	base: fieldSurface,
	variants: {
		size: {
			xs: 'h-6 px-1.5 text-xs',
			sm: 'h-7 px-2 text-sm',
			md: 'h-8 px-2.5 text-sm'
		},
		mono: { true: 'font-mono text-xs', false: '' }
	},
	defaultVariants: { size: 'sm', mono: false }
});

export const textarea = tv({
	base: `${fieldSurface} min-h-16 px-2 py-1.5 text-sm leading-relaxed`,
	variants: { mono: { true: 'font-mono text-xs', false: '' } },
	defaultVariants: { mono: false }
});

export const numberField = tv({
	slots: {
		root: 'min-w-0',
		group: `flex w-full min-w-0 items-stretch overflow-hidden rounded-md border border-border-strong/70 bg-depth-0 sunken
			transition-[border-color,box-shadow] duration-100
			hover:border-border-strong
			data-focus-within:border-ring data-focus-within:ring-2 data-focus-within:ring-ring/25
			data-invalid:border-danger data-disabled:opacity-50`,
		input: 'num w-full min-w-0 border-0 bg-transparent text-right text-foreground outline-none',
		unit: 'flex select-none items-center pr-2 text-2xs text-subtle-foreground',
		stepper: `flex w-6 shrink-0 items-center justify-center text-muted-foreground transition-colors
			hover:bg-depth-2 hover:text-foreground data-pressed:bg-depth-3
			data-disabled:pointer-events-none data-disabled:opacity-30 [&_svg]:size-3`
	},
	variants: {
		size: {
			xs: { group: 'h-6', input: 'px-1.5 text-xs' },
			sm: { group: 'h-7', input: 'px-2 text-sm' },
			md: { group: 'h-8', input: 'px-2.5 text-sm' }
		}
	},
	defaultVariants: { size: 'sm' }
});

export const select = tv({
	slots: {
		trigger: `${fieldSurface} inline-flex items-center justify-between gap-1.5 text-left
			aria-expanded:border-ring aria-expanded:ring-2 aria-expanded:ring-ring/25
			[&>svg]:size-3.5 [&>svg]:shrink-0 [&>svg]:text-subtle-foreground`,
		value: 'min-w-0 flex-1 truncate',
		placeholder: 'min-w-0 flex-1 truncate text-subtle-foreground'
	},
	variants: {
		size: {
			xs: { trigger: 'h-6 px-1.5 text-xs' },
			sm: { trigger: 'h-7 px-2 text-sm' },
			md: { trigger: 'h-8 px-2.5 text-sm' }
		}
	},
	defaultVariants: { size: 'sm' }
});

/** Floating surfaces: popovers, menus and the list of a select. */
export const floating = tv({
	slots: {
		content: `z-50 rounded-lg border bg-depth-0 p-1 text-sm text-foreground shadow-lg outline-none
			data-entering:animate-in data-entering:fade-in-0 data-entering:zoom-in-98 data-entering:[animation-duration:90ms]
			data-exiting:animate-out data-exiting:fade-out-0 data-exiting:zoom-out-98 data-exiting:[animation-duration:90ms]`,
		list: 'flex max-h-72 flex-col gap-px overflow-y-auto outline-none',
		item: `relative flex min-h-7 cursor-default select-none items-center gap-2 rounded-sm px-2 py-1 text-sm outline-none
			hover:bg-depth-2 data-focused:bg-depth-2 data-highlighted:bg-depth-2
			data-selected:font-medium data-selected:text-primary-soft-foreground
			data-disabled:pointer-events-none data-disabled:opacity-40
			[&_svg]:size-3.5 [&_svg]:shrink-0`,
		itemHint: 'ml-auto pl-3 text-xs font-normal text-subtle-foreground',
		separator: '-mx-1 my-1 h-px bg-border',
		label: 'px-2 pt-1.5 pb-1 text-2xs font-semibold tracking-wide text-subtle-foreground uppercase'
	}
});

export const checkbox = tv({
	slots: {
		root: `inline-flex size-4 shrink-0 items-center justify-center rounded-sm border border-border-strong bg-depth-0 sunken
			text-primary-foreground transition-colors duration-75 ${focusRing}
			hover:border-primary/60
			data-checked:border-primary data-checked:bg-primary
			data-indeterminate:border-primary data-indeterminate:bg-primary
			data-disabled:cursor-not-allowed data-disabled:opacity-45`,
		indicator: 'inline-flex items-center justify-center [&_svg]:size-3 [&_svg]:stroke-[3]',
		label: 'inline-flex cursor-pointer select-none items-center gap-2 text-sm'
	}
});

export const switchRecipe = tv({
	slots: {
		root: `inline-flex h-4.5 w-8 shrink-0 items-center rounded-full border border-border-strong/70 bg-depth-3 p-px sunken
			transition-colors duration-100 ${focusRing}
			data-checked:border-primary data-checked:bg-primary
			data-disabled:cursor-not-allowed data-disabled:opacity-45`,
		thumb: `block size-3.5 rounded-full bg-depth-0 shadow-sm transition-transform duration-150 ease-out
			data-checked:translate-x-3.5`,
		label: 'inline-flex cursor-pointer select-none items-center gap-2 text-sm'
	}
});

export const dialog = tv({
	slots: {
		overlay: `fixed inset-0 z-50 bg-foreground/25 backdrop-blur-[2px]
			data-entering:animate-in data-entering:fade-in-0 data-exiting:animate-out data-exiting:fade-out-0`,
		content: `relative flex max-h-[calc(100dvh-3rem)] w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-xl border bg-depth-0 shadow-xl outline-none
			data-entering:animate-in data-entering:fade-in-0 data-entering:zoom-in-97 data-entering:[animation-duration:120ms]
			data-exiting:animate-out data-exiting:fade-out-0 data-exiting:zoom-out-97 data-exiting:[animation-duration:100ms]`,
		header: 'flex shrink-0 items-start gap-3 border-b bg-depth-1 px-4 py-3',
		title: 'text-base font-semibold text-foreground',
		description: 'mt-0.5 text-xs text-muted-foreground',
		body: 'min-h-0 flex-1 overflow-y-auto',
		footer: 'flex shrink-0 items-center justify-end gap-2 border-t bg-depth-1 px-4 py-2.5'
	},
	variants: {
		size: {
			sm: { content: 'max-w-sm' },
			md: { content: 'max-w-lg' },
			lg: { content: 'max-w-3xl' },
			xl: { content: 'max-w-6xl' }
		}
	},
	defaultVariants: { size: 'md' }
});

/** A titled block of a side panel, optionally collapsible. */
export const section = tv({
	slots: {
		root: 'border-b last:border-b-0',
		header: `group flex h-9 w-full select-none items-center gap-1.5 px-3 text-left ${focusRing}`,
		title: 'text-2xs font-semibold tracking-[0.06em] text-muted-foreground uppercase',
		count: 'num text-2xs text-subtle-foreground',
		chevron: 'size-3.5 text-subtle-foreground transition-transform duration-150 group-aria-expanded:rotate-90',
		actions: 'ml-auto flex items-center gap-1',
		body: 'px-3 pb-3'
	}
});

export const fieldRecipe = tv({
	slots: {
		root: 'flex min-w-0 flex-col gap-1',
		label: 'text-xs font-medium text-muted-foreground',
		hint: 'text-2xs leading-snug text-subtle-foreground',
		error: 'text-2xs leading-snug text-danger'
	},
	variants: {
		layout: {
			stack: {},
			inline: { root: 'grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] items-center gap-x-3 gap-y-0.5' }
		}
	},
	defaultVariants: { layout: 'stack' }
});

export const badge = tv({
	base: 'num inline-flex h-5 shrink-0 items-center gap-1 whitespace-nowrap rounded-sm px-1.5 text-2xs font-medium [&_svg]:size-3',
	variants: {
		tone: {
			neutral: 'bg-depth-3 text-muted-foreground',
			primary: 'bg-primary-soft text-primary-soft-foreground',
			success: 'bg-success-soft text-success',
			warning: 'bg-warning-soft text-warning',
			danger: 'bg-danger-soft text-danger',
			info: 'bg-info-soft text-info'
		},
		outline: { true: 'border bg-transparent', false: '' }
	},
	defaultVariants: { tone: 'neutral', outline: false }
});
export type BadgeVariants = VariantProps<typeof badge>;

export const card = tv({
	base: 'rounded-lg border bg-depth-0 raised',
	variants: {
		interactive: {
			true: `cursor-pointer text-left transition-[border-color,box-shadow] duration-100 hover:border-border-strong hover:shadow-md ${focusRing}`,
			false: ''
		},
		selected: { true: 'border-primary ring-2 ring-primary/20', false: '' }
	},
	defaultVariants: { interactive: false, selected: false }
});

export const table = tv({
	slots: {
		root: 'w-full border-separate border-spacing-0 text-xs',
		th: `sticky top-0 z-[1] h-7 border-b bg-depth-1 px-2 text-left text-2xs font-semibold tracking-wide whitespace-nowrap
			text-muted-foreground uppercase`,
		td: 'h-7 border-b border-border/70 px-2 align-middle',
		tr: 'transition-colors hover:bg-depth-1 data-selected:bg-primary-soft/60'
	}
});

/** A short label next to a control; dark so it reads over the 3D view too. */
export const tooltip = tv({
	base: `pointer-events-none fixed top-0 left-0 z-[60] max-w-64 rounded-md bg-foreground px-2 py-1
		text-xs leading-snug text-depth-0 shadow-md
		animate-in fade-in-0 zoom-in-95 [animation-duration:80ms]`
});

export const kbd = tv({
	base: 'inline-flex h-4.5 min-w-4.5 items-center justify-center rounded-sm border border-b-2 bg-depth-0 px-1 font-mono text-2xs text-muted-foreground'
});

/** A row of colour samples to pick one: a board decor, a finish. */
export const swatches = tv({
	slots: {
		root: 'flex flex-wrap items-center gap-1.5',
		item: `relative size-6 shrink-0 rounded-full border border-border-strong shadow-xs outline-none
			transition-[box-shadow,transform] duration-100 hover:scale-110
			data-selected:ring-2 data-selected:ring-primary data-selected:ring-offset-2 data-selected:ring-offset-depth-0
			data-focus-visible:ring-2 data-focus-visible:ring-ring/60 data-focus-visible:ring-offset-2 data-focus-visible:ring-offset-depth-0
			data-disabled:pointer-events-none data-disabled:opacity-40`,
		/** A print with a direction (wood): faint stripes over the colour. */
		grain: `pointer-events-none absolute inset-0 rounded-full opacity-35
			bg-[repeating-linear-gradient(100deg,transparent_0_3px,rgb(0_0_0/0.35)_3px_4px)]`
	}
});

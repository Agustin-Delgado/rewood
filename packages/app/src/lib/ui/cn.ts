import { twMerge } from 'tailwind-merge';

type ClassValue = string | false | null | undefined;

/** Joins classes, letting a later utility win over an earlier one of the same kind. */
export function cn(...values: ClassValue[]): string {
	return twMerge(values.filter(Boolean).join(' '));
}

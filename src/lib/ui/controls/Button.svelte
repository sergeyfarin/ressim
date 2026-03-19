<script lang="ts">
    import type { HTMLButtonAttributes } from "svelte/elements";

    let {
        children,
        class: className,
        variant = "default",
        size = "default",
        ...rest
    }: HTMLButtonAttributes & {
        variant?:
            | "default"
            | "destructive"
            | "outline"
            | "secondary"
            | "ghost"
            | "link"
            | "success";
        size?: "default" | "sm" | "lg" | "icon";
    } = $props();

    const baseStyles =
        "inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50";

    const variants = {
        default:
            "bg-primary text-primary-foreground shadow hover:bg-primary/90",
        destructive:
            "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90",
        outline:
            "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground",
        secondary:
            "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground",
        link: "text-primary underline-offset-4 hover:underline",
        success: "bg-green-600 text-white shadow hover:bg-green-500",
    };

    const sizes = {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md px-3 text-xs",
        lg: "h-10 rounded-md px-8",
        icon: "h-9 w-9",
    };
</script>

<button
    class="{baseStyles} {variants[variant]} {sizes[size]} {className}"
    {...rest}
>
    {@render children?.()}
</button>

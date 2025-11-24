import { Button as UIButton, type ButtonProps } from "./ui/button.tsx";
import { IS_BROWSER } from "$fresh/runtime.ts";

export function Button(props: ButtonProps) {
  return (
    <UIButton
      {...props}
      disabled={!IS_BROWSER || props.disabled}
    />
  );
}

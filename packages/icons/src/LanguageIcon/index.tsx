import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";

import type { IconProps } from "../icon";
import CSharpIcon from "./c-sharp-icon";
import CodeIcon from "./code-icon";
import CppIcon from "./cpp-icon";
import ErlangIcon from "./erlang-icon";
import GoIcon from "./go-icon";
import JavaIcon from "./java-icon";
import JavaScriptIcon from "./java-script-icon";
import PhpIcon from "./php-icon";
import PythonIcon from "./python-icon";
import RubyIcon from "./ruby-icon";
import RustIcon from "./rust-icon";
import SwiftIcon from "./swift-icon";
import TypeScriptIcon from "./type-script-icon";

const LANGUAGES: Record<string, Language> = {
  rust: { name: "Rust", icon: RustIcon },
  go: { name: "Go", icon: GoIcon },
  python: { name: "Python", icon: PythonIcon },
  javascript: { name: "JavaScript", icon: JavaScriptIcon },
  typescript: { name: "TypeScript", icon: TypeScriptIcon },
  java: { name: "Java", icon: JavaIcon },
  csharp: { name: "C#", icon: CSharpIcon },
  cpp: { name: "C++", icon: CppIcon },
  ruby: { name: "Ruby", icon: RubyIcon },
  php: { name: "PHP", icon: PhpIcon },
  swift: { name: "Swift", icon: SwiftIcon },
  erlang: { name: "Erlang", icon: ErlangIcon },
};

interface Language {
  readonly name: string;
  readonly icon: Component<IconProps>;
}

interface LanguageIconProps extends IconProps {
  readonly language: string | undefined;
}

export default function LanguageIcon(props: LanguageIconProps) {
  const knownLanguage = () =>
    props.language === undefined ? undefined : LANGUAGES[props.language];
  return (
    <Dynamic
      component={knownLanguage()?.icon ?? CodeIcon}
      size={props.size}
      class={knownLanguage() ? props.class : `text-muted ${props.class ?? ""}`}
      title={props.title ?? toLanguageName(props.language)}
    />
  );
}

export function toLanguageName(languageId: string | undefined): string {
  return languageId === undefined
    ? "Unknown language"
    : (LANGUAGES[languageId]?.name ?? languageId);
}

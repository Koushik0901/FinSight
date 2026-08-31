import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";
import "./styles/vars.css";
import "./styles/custom.css";

import FeatureCard from "./components/FeatureCard.vue";
import FeatureGrid from "./components/FeatureGrid.vue";
import HomeHero from "./components/HomeHero.vue";
import Philosophy from "./components/Philosophy.vue";
import PrivacyCallout from "./components/PrivacyCallout.vue";
import StepList from "./components/StepList.vue";
import ProviderCard from "./components/ProviderCard.vue";
import ScreenshotFrame from "./components/ScreenshotFrame.vue";
import Icon from "./components/Icon.vue";
export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component("FeatureCard", FeatureCard);
    app.component("FeatureGrid", FeatureGrid);
    app.component("HomeHero", HomeHero);
    app.component("Philosophy", Philosophy);
    app.component("PrivacyCallout", PrivacyCallout);
    app.component("StepList", StepList);
    app.component("ProviderCard", ProviderCard);
    app.component("ScreenshotFrame", ScreenshotFrame);
    app.component("Icon", Icon);
  },
} satisfies Theme;

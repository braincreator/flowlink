import React from "react";
import { Composition } from "remotion";
import { TerminalHero, TERMINAL_HERO_CONFIG } from "./TerminalHero";
import {
  CaseDropTable,
  CASE_DROP_TABLE,
  CaseDockerRm,
  CASE_DOCKER_RM,
  CaseGitReset,
  CASE_GIT_RESET,
  CaseChmod777,
  CASE_CHMOD_777,
} from "./CaseStudies";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="TerminalHero"
        component={TerminalHero}
        durationInFrames={TERMINAL_HERO_CONFIG.durationInFrames}
        fps={TERMINAL_HERO_CONFIG.fps}
        width={TERMINAL_HERO_CONFIG.width}
        height={TERMINAL_HERO_CONFIG.height}
      />
      <Composition
        id="CaseDropTable"
        component={CaseDropTable}
        durationInFrames={CASE_DROP_TABLE.durationInFrames}
        fps={CASE_DROP_TABLE.fps}
        width={CASE_DROP_TABLE.width}
        height={CASE_DROP_TABLE.height}
      />
      <Composition
        id="CaseDockerRm"
        component={CaseDockerRm}
        durationInFrames={CASE_DOCKER_RM.durationInFrames}
        fps={CASE_DOCKER_RM.fps}
        width={CASE_DOCKER_RM.width}
        height={CASE_DOCKER_RM.height}
      />
      <Composition
        id="CaseGitReset"
        component={CaseGitReset}
        durationInFrames={CASE_GIT_RESET.durationInFrames}
        fps={CASE_GIT_RESET.fps}
        width={CASE_GIT_RESET.width}
        height={CASE_GIT_RESET.height}
      />
      <Composition
        id="CaseChmod777"
        component={CaseChmod777}
        durationInFrames={CASE_CHMOD_777.durationInFrames}
        fps={CASE_CHMOD_777.fps}
        width={CASE_CHMOD_777.width}
        height={CASE_CHMOD_777.height}
      />
    </>
  );
};

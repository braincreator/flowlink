import { Composition } from "remotion";
import { FlowLinkPromo } from "./FlowLinkPromo";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="FlowLinkPromo"
        component={FlowLinkPromo}
        durationInFrames={1800}
        fps={30}
        width={1920}
        height={1080}
      />
    </>
  );
};

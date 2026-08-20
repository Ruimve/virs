import { Suspense } from 'react';
import { Outlet } from 'react-router-dom';
import Fallback from '@/components/Transition/Fallback';
import { Layout } from '@/layout';
import { WizardProvider } from '../context/WizardContext/WizardProvider';
import Header from './Header';

export const LayoutBox = () => {
  return (
    <WizardProvider>
      <Suspense fallback={<Fallback label="正在加载页面..." startProgress={65} progress={90} />}>
        <Layout header={<Header />} sidebar={false}>
          <Outlet />
        </Layout>
      </Suspense>
    </WizardProvider>
  );
};

export default LayoutBox;

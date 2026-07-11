import { Module } from '@nestjs/common';
import { AffinityService } from './affinity.service';
@Module({ providers: [AffinityService], exports: [AffinityService] })
export class AffinityModule {}

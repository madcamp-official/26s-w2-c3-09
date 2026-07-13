import { Injectable, ServiceUnavailableException } from '@nestjs/common';
import {
  DeleteObjectCommand,
  GetObjectCommand,
  HeadObjectCommand,
  PutObjectCommand,
  S3Client,
  type S3ClientConfig,
} from '@aws-sdk/client-s3';
import { getSignedUrl } from '@aws-sdk/s3-request-presigner';
import { z } from 'zod';

const optionalString = z.preprocess(
  (value) => (value === '' ? undefined : value),
  z.string().min(1).optional(),
);

const schema = z
  .object({
    OBJECT_STORAGE_ENDPOINT: z.preprocess(
      (value) => (value === '' ? undefined : value),
      z.url().optional(),
    ),
    OBJECT_STORAGE_REGION: z.string().min(1),
    OBJECT_STORAGE_BUCKET: z.string().min(1),
    OBJECT_STORAGE_ACCESS_KEY_ID: optionalString,
    OBJECT_STORAGE_SECRET_ACCESS_KEY: optionalString,
  })
  .superRefine((value, context) => {
    const hasAccessKey = Boolean(value.OBJECT_STORAGE_ACCESS_KEY_ID);
    const hasSecretKey = Boolean(value.OBJECT_STORAGE_SECRET_ACCESS_KEY);
    if (hasAccessKey !== hasSecretKey) {
      context.addIssue({
        code: 'custom',
        path: [
          hasAccessKey
            ? 'OBJECT_STORAGE_SECRET_ACCESS_KEY'
            : 'OBJECT_STORAGE_ACCESS_KEY_ID',
        ],
        message:
          'object storage static credentials must be configured together',
      });
    }
  });

export type ObjectStorageConfig = z.infer<typeof schema>;

export function loadObjectStorageConfig(
  source: NodeJS.ProcessEnv = process.env,
) {
  const result = schema.safeParse(source);
  if (!result.success)
    throw new ServiceUnavailableException({
      code: 'UNCONFIGURED',
      provider: 'OBJECT_STORAGE',
    });
  return result.data;
}

export function objectStorageClientOptions(
  config: ObjectStorageConfig,
): S3ClientConfig {
  const staticCredentials =
    config.OBJECT_STORAGE_ACCESS_KEY_ID &&
    config.OBJECT_STORAGE_SECRET_ACCESS_KEY
      ? {
          accessKeyId: config.OBJECT_STORAGE_ACCESS_KEY_ID,
          secretAccessKey: config.OBJECT_STORAGE_SECRET_ACCESS_KEY,
        }
      : undefined;
  return {
    region: config.OBJECT_STORAGE_REGION,
    ...(config.OBJECT_STORAGE_ENDPOINT
      ? {
          endpoint: config.OBJECT_STORAGE_ENDPOINT,
          forcePathStyle: true,
        }
      : {}),
    ...(staticCredentials ? { credentials: staticCredentials } : {}),
  };
}

@Injectable()
export class ObjectStorageService {
  private config() {
    return loadObjectStorageConfig();
  }
  private client() {
    const c = this.config();
    return {
      config: c,
      client: new S3Client(objectStorageClientOptions(c)),
    };
  }
  assertConfigured() {
    this.config();
  }
  async uploadUrl(key: string, expiresIn: number) {
    const { client, config } = this.client();
    return getSignedUrl(
      client,
      new PutObjectCommand({ Bucket: config.OBJECT_STORAGE_BUCKET, Key: key }),
      { expiresIn },
    );
  }
  async downloadUrl(key: string, expiresIn: number) {
    const { client, config } = this.client();
    return getSignedUrl(
      client,
      new GetObjectCommand({ Bucket: config.OBJECT_STORAGE_BUCKET, Key: key }),
      { expiresIn },
    );
  }
  async size(key: string) {
    const { client, config } = this.client();
    return (
      await client.send(
        new HeadObjectCommand({
          Bucket: config.OBJECT_STORAGE_BUCKET,
          Key: key,
        }),
      )
    ).ContentLength;
  }
  async delete(key: string) {
    const { client, config } = this.client();
    await client.send(
      new DeleteObjectCommand({
        Bucket: config.OBJECT_STORAGE_BUCKET,
        Key: key,
      }),
    );
  }
}

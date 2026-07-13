import {
  loadObjectStorageConfig,
  objectStorageClientOptions,
} from './object-storage.service';

describe('object storage configuration', () => {
  it('uses the AWS SDK credential chain when static credentials are absent', () => {
    const config = loadObjectStorageConfig({
      OBJECT_STORAGE_REGION: 'ap-southeast-2',
      OBJECT_STORAGE_BUCKET: 'mousekeeper-private',
    });

    expect(objectStorageClientOptions(config)).toEqual({
      region: 'ap-southeast-2',
    });
  });

  it('keeps endpoint and static credentials for other S3-compatible providers', () => {
    const config = loadObjectStorageConfig({
      OBJECT_STORAGE_ENDPOINT: 'https://objects.example.com',
      OBJECT_STORAGE_REGION: 'auto',
      OBJECT_STORAGE_BUCKET: 'mousekeeper-private',
      OBJECT_STORAGE_ACCESS_KEY_ID: 'access-key',
      OBJECT_STORAGE_SECRET_ACCESS_KEY: 'secret-key',
    });

    expect(objectStorageClientOptions(config)).toEqual({
      endpoint: 'https://objects.example.com',
      forcePathStyle: true,
      region: 'auto',
      credentials: {
        accessKeyId: 'access-key',
        secretAccessKey: 'secret-key',
      },
    });
  });

  it('rejects an incomplete static credential pair', () => {
    expect(() =>
      loadObjectStorageConfig({
        OBJECT_STORAGE_REGION: 'ap-southeast-2',
        OBJECT_STORAGE_BUCKET: 'mousekeeper-private',
        OBJECT_STORAGE_ACCESS_KEY_ID: 'access-key',
      }),
    ).toThrow('Service Unavailable');
  });
});
